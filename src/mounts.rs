use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fuser::{BackgroundSession, INodeNo, MountOption, Notifier, SessionACL};
use rustix::fs::{Mode, OFlags};

use crate::api::{MountParams, MountType, MountsItem, OverlayParams};
use crate::state::{self, StoredMount};
use crate::view::{Config, Fs, Individual, Invalidation, Shared, Squash, View};

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
}

impl Registry {
    pub fn new(data_dir: PathBuf, stored: Vec<StoredMount>) -> Self {
        Self {
            data_dir,
            mounts: BTreeMap::new(),
            stored: stored
                .into_iter()
                .map(|item| (item.dst().to_owned(), item))
                .collect(),
        }
    }

    pub fn restore(&mut self) {
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
            rustix::mount::unmount(&earlier.mount_point, rustix::mount::UnmountFlags::DETACH)
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
        rustix::mount::unmount(&dst, rustix::mount::UnmountFlags::DETACH)
            .map_err(|err| MountError::Failed(format!("unmount {}: {err}", dst.display())))?;
        let mounted = self.mounts.remove(&dst).ok_or_else(not_served)?;
        // fuser waits up to 5 seconds for open files when it drops a session.
        std::thread::spawn(move || drop(mounted));
        Ok(())
    }

    pub fn mount(&mut self, params: MountParams) -> Result<(), MountError> {
        let src = existing_dir("src", &params.src)?;
        let dst = existing_dir("dst", &params.dst)?;
        if params.type_ == MountType::Pet && params.overlay.is_some() {
            return Err(MountError::InvalidParams(
                "overlay is not allowed for type pet".into(),
            ));
        }
        let individual = match params
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.individual.as_ref())
        {
            Some(individual) => {
                for name in &individual.directories {
                    check_name(name)?;
                }
                Some((
                    existing_dir("overlay.individual.src", &individual.src)?,
                    &individual.directories,
                ))
            }
            None => None,
        };
        let shared = match params
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.shared.as_ref())
        {
            Some(shared) => Some(existing_dir("overlay.shared.src", &shared.src)?),
            None => None,
        };
        check_apart(&dst, "src", &src)?;
        if let Some((individual_src, _)) = &individual {
            check_apart(&dst, "overlay.individual.src", individual_src)?;
        }
        if let Some(shared_src) = &shared {
            check_apart(&dst, "overlay.shared.src", shared_src)?;
        }
        for served in self.mounts.keys().filter(|served| **served != dst) {
            check_apart(&dst, "a served dst", served)?;
        }

        let config = Config {
            src: Some(open_dir(&src)?),
            individual: match individual {
                Some((individual_src, directories)) => {
                    Some(open_individual(&individual_src, directories)?)
                }
                None => None,
            },
            shared: match shared {
                Some(shared_src) => Some(Shared {
                    src: open_dir(&shared_src)?,
                }),
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
        state::save(&self.data_dir, self.stored.values().cloned().collect())
            .map_err(|err| MountError::Failed(format!("write the state file: {err}")))
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
            individual: None,
            shared: None,
            squash: *squash,
        }),
        StoredMount::Cattle {
            overlay, squash, ..
        } => {
            let individual = match &overlay.individual {
                Some(individual) => {
                    for name in &individual.directories {
                        check_name(name)?;
                    }
                    Some(open_individual(&individual.src, &individual.directories)?)
                }
                None => None,
            };
            let shared = match &overlay.shared {
                Some(shared) => Some(Shared {
                    src: open_dir(&shared.src)?,
                }),
                None => None,
            };
            Ok(Config {
                src: None,
                individual,
                shared,
                squash: *squash,
            })
        }
    }
}

fn open_individual(src: &Path, directories: &[String]) -> Result<Individual, MountError> {
    Ok(Individual {
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

fn check_name(name: &str) -> Result<(), MountError> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0']) {
        return Err(MountError::InvalidParams(format!(
            "overlay.individual.directories item {name:?} must be a plain name"
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
