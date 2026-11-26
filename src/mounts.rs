use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fuser::{BackgroundSession, INodeNo, MountOption, Notifier, SessionACL};
use rustix::fs::{Mode, OFlags};

use crate::api::MountParams;
use crate::view::{Config, Fs, Invalidation, Overlay, View};

pub enum MountError {
    InvalidParams(String),
    Failed(String),
}

struct Mounted {
    _session: BackgroundSession,
    notifier: Notifier,
    view: Arc<View>,
}

#[derive(Default)]
pub struct Registry {
    mounts: HashMap<PathBuf, Mounted>,
}

impl Registry {
    pub fn mount(&mut self, params: MountParams) -> Result<(), MountError> {
        let src = existing_dir("src", &params.src)?;
        let dst = existing_dir("dst", &params.dst)?;
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
            src: open_dir(&src)?,
            overlay: match overlay {
                Some((overlay_src, directories)) => Some(Overlay {
                    src: open_dir(&overlay_src)?,
                    directories: directories.iter().map(OsString::from).collect(),
                }),
                None => None,
            },
            squash: params.squash,
        };

        if let Some(mounted) = self.mounts.get(&dst) {
            invalidate(&mounted.notifier, mounted.view.replace(config));
            return Ok(());
        }
        if let Some((mount_point, fs_type)) = foreign_mount(&dst)? {
            return Err(MountError::Failed(format!(
                "{} holds a {fs_type} mount",
                mount_point.display()
            )));
        }
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
            },
        );
        Ok(())
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

fn foreign_mount(dst: &Path) -> Result<Option<(PathBuf, String)>, MountError> {
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|err| MountError::Failed(format!("read /proc/self/mountinfo: {err}")))?;
    Ok(parse_mountinfo(&mountinfo)
        .into_iter()
        .find(|(mount_point, _)| mount_point.starts_with(dst)))
}

fn parse_mountinfo(mountinfo: &str) -> Vec<(PathBuf, String)> {
    mountinfo
        .lines()
        .filter_map(|line| {
            let (mount, fs) = line.split_once(" - ")?;
            let mount_point = mount.split(' ').nth(4)?;
            let fs_type = fs.split(' ').next()?;
            Some((PathBuf::from(unescape(mount_point)), fs_type.to_owned()))
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
                (PathBuf::from("/mnt/my dir"), "ext3".to_owned()),
                (PathBuf::from("/proc"), "proc".to_owned()),
            ]
        );
    }

    #[test]
    fn mountinfo_with_optional_fields() {
        let mountinfo = "100 1 0:50 / /d rw shared:5 master:2 - fuse.bindfs bindfs rw\n";
        assert_eq!(
            parse_mountinfo(mountinfo),
            vec![(PathBuf::from("/d"), "fuse.bindfs".to_owned())]
        );
    }
}
