use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use fuser::{BackgroundSession, INodeNo, MountOption, Notifier, SessionACL};
use rustix::fs::{CWD, Gid, Mode, OFlags, Uid};
use rustix::mount::{MoveMountFlags, OpenTreeFlags, UnmountFlags};

use crate::api::{BindParams, MountParams, MountType, MountsItem, OverlayParams};
use crate::state::{self, StoredBind, StoredMount};
use crate::view::{Config, Fs, Invalidation, Overlay, Squash, View};

pub enum MountError {
    InvalidParams(String),
    Failed(String),
}

impl fmt::Display for MountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParams(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

struct Mounted {
    _session: BackgroundSession,
    notifier: Notifier,
    view: Arc<View>,
    shown: Shown,
}

#[derive(Clone, PartialEq)]
struct Bind {
    point: PathBuf,
    entry: PathBuf,
    dir: bool,
}

#[derive(Clone)]
struct Shown {
    type_: MountType,
    src: Option<PathBuf>,
    overlay: Option<OverlayParams>,
    squash: Option<Squash>,
}

pub struct Registry {
    data_dir: PathBuf,
    mounts: BTreeMap<PathBuf, Mounted>,
    stored: BTreeMap<PathBuf, StoredMount>,
    binds: BTreeMap<PathBuf, Vec<Bind>>,
    stored_binds: BTreeMap<PathBuf, PathBuf>,
}

impl Registry {
    pub fn new(data_dir: PathBuf, stored: Vec<StoredMount>, stored_binds: Vec<StoredBind>) -> Self {
        Self {
            data_dir,
            mounts: BTreeMap::new(),
            stored: stored
                .into_iter()
                .map(|item| (item.dst().to_owned(), item))
                .collect(),
            binds: BTreeMap::new(),
            stored_binds: stored_binds
                .into_iter()
                .map(|item| (item.dst, item.src))
                .collect(),
        }
    }

    pub fn restore(&mut self) {
        let pairs: Vec<(PathBuf, PathBuf)> = self
            .stored_binds
            .iter()
            .map(|(dst, src)| (dst.clone(), src.clone()))
            .collect();
        for (dst, src) in pairs {
            if let Err(reason) = self.apply_bind(&dst, &src) {
                eprintln!("bind {}: skipped: {reason}", dst.display());
            }
        }
        let items: Vec<StoredMount> = self.stored.values().cloned().collect();
        for item in items {
            if let Err(reason) = self.restore_item(&item) {
                eprintln!("restore {}: skipped: {reason}", item.dst().display());
            }
        }
    }

    fn restore_item(&mut self, item: &StoredMount) -> Result<(), MountError> {
        let dst = item.dst();
        while let Some(earlier) = mount_entries()?.into_iter().find(|entry| {
            entry.mount_point == dst && entry.fs_type == "fuse" && entry.source == "vm-herd-fs"
        }) {
            rustix::mount::unmount(&earlier.mount_point, UnmountFlags::DETACH)
                .map_err(|err| MountError::Failed(format!("detach the earlier mount: {err}")))?;
        }
        if !dst.is_dir() {
            return Err(MountError::Failed("dst is not a directory".into()));
        }
        if let Some(entry) = foreign_mount(dst)? {
            return Err(MountError::Failed(format!(
                "{} holds a {} mount",
                entry.mount_point.display(),
                entry.fs_type
            )));
        }
        self.attach(dst.to_owned(), restored_config(item)?, restored_shown(item))
    }

    pub fn mounts(&self) -> Vec<MountsItem> {
        self.mounts
            .iter()
            .map(|(dst, mounted)| MountsItem {
                type_: mounted.shown.type_,
                src: mounted.shown.src.clone(),
                overlay: mounted.shown.overlay.clone(),
                dst: dst.clone(),
                squash: mounted.shown.squash,
            })
            .collect()
    }

    pub fn umount(&mut self, dst: &Path) -> Result<(), MountError> {
        if !dst.is_absolute() {
            return Err(MountError::InvalidParams(format!(
                "dst must be an absolute path, got {}",
                dst.display()
            )));
        }
        let not_served = || MountError::Failed(format!("dst {} is not served", dst.display()));
        let dst = std::fs::canonicalize(dst).map_err(|_| not_served())?;
        let mounted = self.mounts.get_mut(&dst).ok_or_else(not_served)?;
        match self.stored.get(&dst) {
            Some(StoredMount::Pet { .. }) => {
                return Err(MountError::Failed(format!(
                    "dst {} is a pet view",
                    dst.display()
                )));
            }
            Some(item) => {
                invalidate(
                    &mounted.notifier,
                    mounted.view.replace(restored_config(item)?),
                );
                mounted.shown = restored_shown(item);
                return Ok(());
            }
            None => {}
        }
        rustix::mount::unmount(&dst, UnmountFlags::DETACH)
            .map_err(|err| MountError::Failed(format!("unmount {}: {err}", dst.display())))?;
        let mounted = self.mounts.remove(&dst).ok_or_else(not_served)?;
        // fuser waits up to 5 seconds for open files when it drops a session.
        std::thread::spawn(move || drop(mounted));
        Ok(())
    }

    pub fn mount(&mut self, params: MountParams) -> Result<(), MountError> {
        let src = existing_dir("src", &params.src)?;
        let dst = dst_dir(&params.dst)?;
        if params.type_ == MountType::Pet && params.overlay.is_some() {
            return Err(MountError::InvalidParams(
                "overlay is not allowed for type pet".into(),
            ));
        }
        let overlay = match &params.overlay {
            Some(overlay) => {
                for name in &overlay.directories {
                    check_name(name)?;
                }
                Some((
                    existing_dir("overlay.src", &overlay.src)?,
                    &overlay.directories,
                ))
            }
            None => None,
        };
        check_apart(&dst, "src", &src)?;
        if let Some((overlay_src, _)) = &overlay {
            check_apart(&dst, "overlay.src", overlay_src)?;
        }
        for served in self.mounts.keys().filter(|served| **served != dst) {
            check_apart(&dst, "a served dst", served)?;
        }

        let config = Config {
            src: Some(open_dir(&src)?),
            overlay: match overlay {
                Some((overlay_src, directories)) => Some(open_overlay(&overlay_src, directories)?),
                None => None,
            },
            squash: params.squash,
        };

        let shown = Shown {
            type_: params.type_,
            src: Some(params.src.clone()),
            overlay: params.overlay.clone(),
            squash: params.squash,
        };
        if let Some(mounted) = self.mounts.get_mut(&dst) {
            invalidate(&mounted.notifier, mounted.view.replace(config));
            mounted.shown = shown;
        } else {
            if let Some(entry) = foreign_mount(&dst)? {
                return Err(MountError::Failed(format!(
                    "{} holds a {} mount",
                    entry.mount_point.display(),
                    entry.fs_type
                )));
            }
            if !dst.exists() {
                create_dst(&dst, params.squash)?;
            }
            self.attach(dst.clone(), config, shown)?;
        }
        let item = match (params.type_, params.overlay) {
            (MountType::Pet, _) => Some(StoredMount::Pet {
                dst: dst.clone(),
                src: params.src,
                squash: params.squash,
            }),
            (MountType::Cattle, Some(overlay)) => Some(StoredMount::Cattle {
                dst: dst.clone(),
                overlay,
                squash: params.squash,
            }),
            (MountType::Cattle, None) => None,
        };
        self.store(dst, item)
    }

    fn store(&mut self, dst: PathBuf, item: Option<StoredMount>) -> Result<(), MountError> {
        match item {
            Some(item) => {
                self.stored.insert(dst, item);
            }
            None => {
                self.stored.remove(&dst);
            }
        }
        self.save()
    }

    pub fn binds(&self) -> Vec<StoredBind> {
        self.stored_binds
            .iter()
            .map(|(dst, src)| StoredBind {
                src: src.clone(),
                dst: dst.clone(),
            })
            .collect()
    }

    fn save(&self) -> Result<(), MountError> {
        state::save(
            &self.data_dir,
            self.stored.values().cloned().collect(),
            self.binds(),
        )
        .map_err(|err| MountError::Failed(format!("write the state file: {err}")))
    }

    pub fn bind(&mut self, params: BindParams) -> Result<(), MountError> {
        let src = existing_dir("src", &params.src)?;
        let dst = existing_dir("dst", &params.dst)?;
        self.apply_bind(&dst, &src)?;
        self.stored_binds.insert(dst, src);
        self.save()
    }

    /// Makes the bind mounts in `dst` match the entries that `src` holds now.
    fn apply_bind(&mut self, dst: &Path, src: &Path) -> Result<(), MountError> {
        let wanted = shared_binds(dst, src)?;
        let known = self.binds.remove(dst).unwrap_or_default();
        let (kept, unwanted): (Vec<Bind>, Vec<Bind>) =
            known.into_iter().partition(|bind| wanted.contains(bind));
        let (replaced, stale): (Vec<Bind>, Vec<Bind>) = unwanted
            .into_iter()
            .partition(|old| wanted.iter().any(|bind| bind.point == old.point));
        // A bind mount on a point that already holds one would stack, and a later detach by
        // path would remove the new one.
        remove_binds(&replaced);
        let missing: Vec<Bind> = wanted
            .iter()
            .filter(|bind| !kept.contains(bind) && !is_bound(bind))
            .cloned()
            .collect();
        if let Err(err) = add_binds(&missing) {
            self.binds
                .insert(dst.to_owned(), kept.into_iter().chain(stale).collect());
            return Err(err);
        }
        remove_binds(&stale);
        self.binds.insert(dst.to_owned(), wanted);
        Ok(())
    }

    pub fn unbind(&mut self, dst: &Path) -> Result<(), MountError> {
        if !dst.is_absolute() {
            return Err(MountError::InvalidParams(format!(
                "dst must be an absolute path, got {}",
                dst.display()
            )));
        }
        // A dst that vm-herd already deleted no longer resolves, but its stored item must go.
        let dst = std::fs::canonicalize(dst).unwrap_or_else(|_| dst.to_owned());
        if !self.binds.contains_key(&dst) && !self.stored_binds.contains_key(&dst) {
            return Err(MountError::Failed(format!(
                "dst {} is not bound",
                dst.display()
            )));
        }
        remove_binds(&self.binds.remove(&dst).unwrap_or_default());
        self.stored_binds.remove(&dst);
        self.save()
    }

    fn attach(&mut self, dst: PathBuf, config: Config, shown: Shown) -> Result<(), MountError> {
        let view = Arc::new(View::new(config));
        let mut options = fuser::Config::default();
        options.mount_options = vec![
            MountOption::FSName("vm-herd-fs".into()),
            MountOption::DefaultPermissions,
            MountOption::NoSuid,
            MountOption::NoDev,
        ];
        options.acl = SessionACL::All;
        let session = fuser::Session::new(Fs(view.clone()), &dst, &options)
            .and_then(fuser::Session::spawn)
            .map_err(|err| MountError::Failed(format!("mount at {}: {err}", dst.display())))?;
        let notifier = session.notifier();
        self.mounts.insert(
            dst,
            Mounted {
                _session: session,
                notifier,
                view,
                shown,
            },
        );
        Ok(())
    }
}

fn restored_config(item: &StoredMount) -> Result<Config, MountError> {
    match item {
        StoredMount::Pet { src, squash, .. } => Ok(Config {
            src: Some(open_dir(src)?),
            overlay: None,
            squash: *squash,
        }),
        StoredMount::Cattle {
            overlay, squash, ..
        } => {
            for name in &overlay.directories {
                check_name(name)?;
            }
            Ok(Config {
                src: None,
                overlay: Some(open_overlay(&overlay.src, &overlay.directories)?),
                squash: *squash,
            })
        }
    }
}

fn open_overlay(src: &Path, directories: &[String]) -> Result<Overlay, MountError> {
    Ok(Overlay {
        src: open_dir(src)?,
        directories: directories.iter().map(OsString::from).collect(),
    })
}

fn restored_shown(item: &StoredMount) -> Shown {
    match item {
        StoredMount::Cattle {
            overlay, squash, ..
        } => Shown {
            type_: MountType::Cattle,
            src: None,
            overlay: Some(overlay.clone()),
            squash: *squash,
        },
        StoredMount::Pet { src, squash, .. } => Shown {
            type_: MountType::Pet,
            src: Some(src.clone()),
            overlay: None,
            squash: *squash,
        },
    }
}

fn existing_dir(param: &str, path: &Path) -> Result<PathBuf, MountError> {
    if !path.is_absolute() {
        return Err(MountError::InvalidParams(format!(
            "{param} must be an absolute path, got {}",
            path.display()
        )));
    }
    let resolved = std::fs::canonicalize(path)
        .map_err(|err| MountError::InvalidParams(format!("{param} {}: {err}", path.display())))?;
    if !resolved.is_dir() {
        return Err(MountError::InvalidParams(format!(
            "{param} {} is not a directory",
            path.display()
        )));
    }
    Ok(resolved)
}

/// A missing `dst` resolves through its parent, so the overlap checks see the path the
/// create will use.
fn dst_dir(path: &Path) -> Result<PathBuf, MountError> {
    if path.exists() {
        return existing_dir("dst", path);
    }
    if !path.is_absolute() {
        return Err(MountError::InvalidParams(format!(
            "dst must be an absolute path, got {}",
            path.display()
        )));
    }
    let wrong =
        |reason: String| MountError::InvalidParams(format!("dst {}: {reason}", path.display()));
    let name = path
        .file_name()
        .ok_or_else(|| wrong("has no name".into()))?;
    let parent = path.parent().ok_or_else(|| wrong("has no parent".into()))?;
    let parent = std::fs::canonicalize(parent).map_err(|err| wrong(err.to_string()))?;
    if !parent.is_dir() {
        return Err(wrong("parent is not a directory".into()));
    }
    Ok(parent.join(name))
}

fn create_dst(dst: &Path, squash: Option<Squash>) -> Result<(), MountError> {
    create_owned(dst, true, squash)
        .map_err(|err| MountError::Failed(format!("create dst {}: {err}", dst.display())))
}

fn create_owned(path: &Path, dir: bool, squash: Option<Squash>) -> rustix::io::Result<()> {
    if dir {
        rustix::fs::mkdir(path, Mode::from_raw_mode(0o755))?;
    } else {
        rustix::fs::open(
            path,
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o644),
        )?;
    }
    if let Some(squash) = squash {
        rustix::fs::chown(
            path,
            Some(Uid::from_raw(squash.uid)),
            Some(Gid::from_raw(squash.gid)),
        )?;
    }
    Ok(())
}

/// Every check runs here, before the caller creates a mount point or adds a bind mount.
fn shared_binds(dst: &Path, src: &Path) -> Result<Vec<Bind>, MountError> {
    let failed =
        |path: &Path, reason: String| MountError::Failed(format!("{}: {reason}", path.display()));
    let entries = std::fs::read_dir(src).map_err(|err| failed(src, err.to_string()))?;
    let mut binds = vec![];
    for entry in entries {
        let entry = entry.map_err(|err| failed(src, err.to_string()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|err| failed(&path, err.to_string()))?;
        if !kind.is_file() && !kind.is_dir() {
            return Err(failed(&path, "is not a file or a directory".into()));
        }
        let point = dst.join(entry.file_name());
        match std::fs::symlink_metadata(&point) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(failed(&point, "is a symlink".into()));
            }
            Ok(meta) if meta.is_dir() != kind.is_dir() => {
                return Err(failed(
                    &point,
                    format!("is not of the same kind as {}", path.display()),
                ));
            }
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(failed(&point, err.to_string())),
        }
        binds.push(Bind {
            point,
            entry: path,
            dir: kind.is_dir(),
        });
    }
    binds.sort_by(|a, b| a.point.cmp(&b.point));
    Ok(binds)
}

/// On a failure, removes the bind mounts that this call added. Created mount points stay.
fn add_binds(binds: &[Bind]) -> Result<(), MountError> {
    for (done, bind) in binds.iter().enumerate() {
        if let Err(err) = add_bind(bind) {
            remove_binds(&binds[..done]);
            return Err(err);
        }
    }
    Ok(())
}

fn add_bind(bind: &Bind) -> Result<(), MountError> {
    let failed = |err: rustix::io::Errno| {
        MountError::Failed(format!(
            "bind {} at {}: {err}",
            bind.entry.display(),
            bind.point.display()
        ))
    };
    if std::fs::symlink_metadata(&bind.point).is_err() {
        create_owned(&bind.point, bind.dir, None).map_err(failed)?;
    }
    let tree = rustix::mount::open_tree(
        CWD,
        &bind.entry,
        OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::AT_SYMLINK_NOFOLLOW,
    )
    .map_err(failed)?;
    // Without MOVE_MOUNT_T_SYMLINKS, a symlink put at the point after the checks fails the
    // call instead of moving the bind mount outside src.
    rustix::mount::move_mount(
        &tree,
        "",
        CWD,
        &bind.point,
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )
    .map_err(failed)
}

fn remove_binds(binds: &[Bind]) {
    if binds.is_empty() {
        return;
    }
    for bind in binds {
        if let Err(err) = detach(&bind.point) {
            eprintln!("detach {}: {err}", bind.point.display());
        }
    }
    // Every host directory binds the same store inode, so nfsd gives the bind mounts of all of
    // them one file handle. Its cache keeps that handle on the detached mount, and clients of
    // the other host directories hang until the cache is flushed.
    match Command::new("exportfs").arg("-f").status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("exportfs -f: {status}"),
        Err(err) => eprintln!("exportfs -f: {err}"),
    }
}

fn detach(point: &Path) -> rustix::io::Result<()> {
    rustix::mount::unmount(point, UnmountFlags::DETACH)?;
    // The removes fail on a non-empty directory and on a point that still holds a mount,
    // which keeps both.
    match std::fs::symlink_metadata(point) {
        Ok(meta) if meta.is_dir() => drop(std::fs::remove_dir(point)),
        Ok(meta) if meta.is_file() && meta.len() == 0 => drop(std::fs::remove_file(point)),
        _ => {}
    }
    Ok(())
}

/// A bind mount that an earlier run left shows the inode of the store entry.
fn is_bound(bind: &Bind) -> bool {
    match (
        std::fs::symlink_metadata(&bind.point),
        std::fs::symlink_metadata(&bind.entry),
    ) {
        (Ok(point), Ok(entry)) => point.dev() == entry.dev() && point.ino() == entry.ino(),
        _ => false,
    }
}

fn check_name(name: &str) -> Result<(), MountError> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0']) {
        return Err(MountError::InvalidParams(format!(
            "overlay.directories item {name:?} must be a plain name"
        )));
    }
    Ok(())
}

fn check_apart(dst: &Path, other_param: &str, other: &Path) -> Result<(), MountError> {
    if dst.starts_with(other) || other.starts_with(dst) {
        return Err(MountError::InvalidParams(format!(
            "dst {} overlaps {other_param} {}",
            dst.display(),
            other.display()
        )));
    }
    Ok(())
}

fn open_dir(path: &Path) -> Result<Arc<OwnedFd>, MountError> {
    rustix::fs::open(
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(Arc::new)
    .map_err(|err| MountError::Failed(format!("open {}: {err}", path.display())))
}

#[derive(Debug, PartialEq)]
struct MountEntry {
    mount_point: PathBuf,
    fs_type: String,
    source: String,
}

fn mount_entries() -> Result<Vec<MountEntry>, MountError> {
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|err| MountError::Failed(format!("read /proc/self/mountinfo: {err}")))?;
    Ok(parse_mountinfo(&mountinfo))
}

fn foreign_mount(dst: &Path) -> Result<Option<MountEntry>, MountError> {
    Ok(mount_entries()?
        .into_iter()
        .find(|entry| entry.mount_point.starts_with(dst)))
}

fn parse_mountinfo(mountinfo: &str) -> Vec<MountEntry> {
    mountinfo
        .lines()
        .filter_map(|line| {
            let (mount, fs) = line.split_once(" - ")?;
            let mount_point = mount.split(' ').nth(4)?;
            let mut fs = fs.split(' ');
            let fs_type = fs.next()?;
            let source = fs.next()?;
            Some(MountEntry {
                mount_point: PathBuf::from(unescape(mount_point)),
                fs_type: fs_type.to_owned(),
                source: unescape(source).to_string_lossy().into_owned(),
            })
        })
        .collect()
}

/// mountinfo writes space, tab, newline, and backslash in paths as `\ooo` octal escapes.
fn unescape(field: &str) -> OsString {
    use std::os::unix::ffi::OsStringExt;
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes.get(i + 1..i + 4).and_then(|digits| {
            std::str::from_utf8(digits)
                .ok()
                .and_then(|digits| u8::from_str_radix(digits, 8).ok())
        });
        match (bytes[i], octal) {
            (b'\\', Some(byte)) => {
                out.push(byte);
                i += 4;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    OsString::from_vec(out)
}

fn invalidate(notifier: &Notifier, invalidation: Invalidation) {
    for (parent, name) in invalidation.names {
        report(notifier.inval_entry(INodeNo(parent), &name));
    }
    for ino in invalidation.inodes {
        report(notifier.inval_inode(INodeNo(ino), 0, 0));
    }
}

fn report(result: io::Result<()>) {
    if let Err(err) = result
        && err.raw_os_error() != Some(libc::ENOENT)
    {
        eprintln!("cache invalidation failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mountinfo_with_escaped_space() {
        let mountinfo = "\
36 35 98:0 / /mnt/my\\040dir rw,noatime master:1 - ext3 /dev/root rw,errors=continue
37 36 0:5 / /proc rw - proc proc rw
";
        assert_eq!(
            parse_mountinfo(mountinfo),
            vec![
                entry("/mnt/my dir", "ext3", "/dev/root"),
                entry("/proc", "proc", "proc"),
            ]
        );
    }

    #[test]
    fn mountinfo_with_optional_fields() {
        let mountinfo = "100 1 0:50 / /d rw shared:5 master:2 - fuse.bindfs bindfs rw\n";
        assert_eq!(
            parse_mountinfo(mountinfo),
            vec![entry("/d", "fuse.bindfs", "bindfs")]
        );
    }

    #[test]
    fn mountinfo_source_of_a_view() {
        let mountinfo = "304 420 0:58 / /tmp/t/d rw,nosuid,nodev,relatime - fuse vm-herd-fs rw,user_id=0,group_id=0,default_permissions,allow_other\n";
        assert_eq!(
            parse_mountinfo(mountinfo),
            vec![entry("/tmp/t/d", "fuse", "vm-herd-fs")]
        );
    }

    fn entry(mount_point: &str, fs_type: &str, source: &str) -> MountEntry {
        MountEntry {
            mount_point: mount_point.into(),
            fs_type: fs_type.into(),
            source: source.into(),
        }
    }
}
