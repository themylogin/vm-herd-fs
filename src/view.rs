use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo, LockOwner,
    OpenFlags, Owner, RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, ReplyXattr, Request, TimeOrNow, WriteFlags,
};
use rustix::fs::{AtFlags, Mode, OFlags, Stat, Timespec, Timestamps};

use crate::nodes::{Key, Node, Nodes, ROOT_ID};

const TTL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Squash {
    pub uid: u32,
    pub gid: u32,
}

pub struct Overlay {
    pub src: Arc<OwnedFd>,
    pub directories: Vec<OsString>,
}

pub struct Config {
    pub src: Option<Arc<OwnedFd>>,
    pub overlay: Option<Overlay>,
    pub squash: Option<Squash>,
}

impl Config {
    fn is_overlay_name(&self, name: &OsStr) -> bool {
        self.overlay
            .as_ref()
            .is_some_and(|overlay| overlay.directories.iter().any(|item| item == name))
    }

    fn is_project(&self, name: &OsStr) -> Res<bool> {
        let Some(src) = &self.src else {
            return Ok(self.overlay_has_project(name));
        };
        match rustix::fs::statat(&**src, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(st) => Ok(is_dir(&st)),
            Err(rustix::io::Errno::NOENT) => Ok(self.overlay_has_project(name)),
            Err(err) => Err(sys(err)),
        }
    }

    fn is_overlay_only_project(&self, name: &OsStr) -> bool {
        let in_src = self.src.as_ref().is_some_and(|src| {
            !matches!(
                rustix::fs::statat(&**src, name, AtFlags::SYMLINK_NOFOLLOW),
                Err(rustix::io::Errno::NOENT)
            )
        });
        !in_src && self.overlay_has_project(name)
    }

    fn overlay_has_project(&self, name: &OsStr) -> bool {
        self.overlay.as_ref().is_some_and(|overlay| {
            rustix::fs::statat(&*overlay.src, name, AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|st| is_dir(&st))
        })
    }

    fn overlay_root(&self) -> Res<&Arc<OwnedFd>> {
        self.overlay
            .as_ref()
            .map(|overlay| &overlay.src)
            .ok_or(Errno::ENOENT)
    }

    fn project_backing(&self, name: &OsStr) -> Option<OwnedFd> {
        self.src
            .as_ref()
            .and_then(|src| open_dir_path(&**src, name).ok())
    }

    fn root_stat(&self) -> Res<Stat> {
        if let Some(src) = &self.src {
            return rustix::fs::fstat(&**src).map_err(sys);
        }
        let mut st = rustix::fs::fstat(&**self.overlay_root()?).map_err(sys)?;
        st.st_mode = libc::S_IFDIR | 0o755;
        Ok(st)
    }

    fn project_stat(&self, name: &OsStr) -> Res<(Option<OwnedFd>, Stat)> {
        if let Some(backing) = self.project_backing(name) {
            let st = rustix::fs::fstat(&backing).map_err(sys)?;
            return Ok((Some(backing), st));
        }
        let overlay = self.overlay.as_ref().ok_or(Errno::ENOENT)?;
        let fd = open_dir_path(&*overlay.src, name).map_err(|_| Errno::ENOENT)?;
        let mut st = rustix::fs::fstat(&fd).map_err(sys)?;
        st.st_mode = libc::S_IFDIR | 0o755;
        Ok((None, st))
    }

    fn overlay_entry(&self, project: &OsStr, name: &OsStr) -> Res<Option<OwnedFd>> {
        let Some(overlay) = &self.overlay else {
            return Ok(None);
        };
        let project_fd = match open_dir_path(&*overlay.src, project) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => return Ok(None),
            Err(err) => return Err(sys(err)),
        };
        match open_path(&project_fd, name) {
            Ok(fd) => Ok(Some(fd)),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(err) => Err(sys(err)),
        }
    }

    fn overlay_parent(&self, project: &OsStr) -> Res<OwnedFd> {
        let overlay = self.overlay.as_ref().ok_or(Errno::ENOENT)?;
        ignore_exists(rustix::fs::mkdirat(
            &*overlay.src,
            project,
            Mode::from_raw_mode(0o755),
        ))?;
        open_dir_path(&*overlay.src, project).map_err(sys)
    }
}

pub struct Invalidation {
    pub names: Vec<(u64, OsString)>,
    pub inodes: Vec<u64>,
}

struct DirEntry {
    id: u64,
    kind: FileType,
    name: OsString,
}

enum Handle {
    File(Arc<File>),
    Dir(Arc<Vec<DirEntry>>),
}

#[derive(Default)]
struct Handles {
    next: u64,
    map: HashMap<u64, Handle>,
}

pub struct View {
    config: RwLock<Arc<Config>>,
    nodes: Mutex<Nodes>,
    handles: Mutex<Handles>,
    cached_names: Mutex<HashSet<(u64, OsString)>>,
}

type Res<T> = Result<T, Errno>;

impl View {
    pub fn new(config: Config) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
            nodes: Mutex::default(),
            handles: Mutex::default(),
            cached_names: Mutex::default(),
        }
    }

    /// Swaps the config and returns the kernel cache entries that may still show the old one.
    pub fn replace(&self, config: Config) -> Invalidation {
        let squash_changed = {
            let mut current = self.config.write().unwrap_or_else(PoisonError::into_inner);
            let changed = current.squash != config.squash;
            *current = Arc::new(config);
            changed
        };
        let names = std::mem::take(&mut *lock(&self.cached_names));
        let nodes = lock(&self.nodes);
        let mut inodes = vec![ROOT_ID];
        inodes.extend(nodes.project_ids());
        if squash_changed {
            inodes.extend(nodes.real_ids());
        }
        Invalidation {
            names: names.into_iter().collect(),
            inodes,
        }
    }

    fn config(&self) -> Arc<Config> {
        self.config
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn node(&self, id: u64) -> Res<Node> {
        lock(&self.nodes).get(id).ok_or(Errno::ENOENT)
    }

    fn record(&self, parent: u64, parent_node: &Node, name: &OsStr) {
        if !matches!(parent_node, Node::Real(_)) {
            lock(&self.cached_names).insert((parent, name.to_owned()));
        }
    }

    fn project_attr(&self, config: &Config, name: &OsStr) -> Res<FileAttr> {
        let id = lock(&self.nodes).project_id(&name.to_owned());
        let (_, st) = config.project_stat(name)?;
        Ok(attr(id, &st, config.squash))
    }

    fn remember(&self, config: &Config, fd: OwnedFd) -> Res<FileAttr> {
        let st = rustix::fs::fstat(&fd).map_err(sys)?;
        let id = lock(&self.nodes).remember(key(&st), fd);
        Ok(attr(id, &st, config.squash))
    }

    fn real_entry(&self, config: &Config, dir: impl AsFd, name: &OsStr) -> Res<FileAttr> {
        let fd = open_path(dir, name).map_err(sys)?;
        self.remember(config, fd)
    }

    fn lookup(&self, parent: u64, name: &OsStr) -> Res<FileAttr> {
        let config = self.config();
        let parent_node = self.node(parent)?;
        let attr = match &parent_node {
            Node::Root => {
                if config.is_project(name)? {
                    self.project_attr(&config, name)?
                } else {
                    let src = config.src.as_ref().ok_or(Errno::ENOENT)?;
                    self.real_entry(&config, &**src, name)?
                }
            }
            Node::Project(project) => {
                if config.is_overlay_name(name) {
                    let fd = config.overlay_entry(project, name)?.ok_or(Errno::ENOENT)?;
                    self.remember(&config, fd)?
                } else {
                    let backing = config.project_backing(project).ok_or(Errno::ENOENT)?;
                    self.real_entry(&config, &backing, name)?
                }
            }
            Node::Real(fd) => self.real_entry(&config, &**fd, name)?,
        };
        self.record(parent, &parent_node, name);
        Ok(attr)
    }

    fn getattr(&self, id: u64) -> Res<FileAttr> {
        let config = self.config();
        match self.node(id)? {
            Node::Root => Ok(attr(ROOT_ID, &config.root_stat()?, config.squash)),
            Node::Project(name) => self.project_attr(&config, &name),
            Node::Real(fd) => {
                let st = rustix::fs::fstat(&*fd).map_err(sys)?;
                Ok(attr(id, &st, config.squash))
            }
        }
    }

    /// The backing object that metadata changes and xattrs apply to.
    fn backing(&self, config: &Config, id: u64) -> Res<Arc<OwnedFd>> {
        match self.node(id)? {
            Node::Root => config.src.clone().ok_or(Errno::EROFS),
            Node::Project(name) => config
                .project_backing(&name)
                .map(Arc::new)
                .ok_or(Errno::EROFS),
            Node::Real(fd) => Ok(fd),
        }
    }

    fn existing_dir(&self, config: &Config, parent: &Node) -> Res<Arc<OwnedFd>> {
        match parent {
            Node::Root => config.src.clone().ok_or(Errno::ENOENT),
            Node::Project(name) => config
                .project_backing(name)
                .map(Arc::new)
                .ok_or(Errno::ENOENT),
            Node::Real(fd) => Ok(fd.clone()),
        }
    }

    fn creatable_dir(&self, config: &Config, parent: &Node, name: &OsStr) -> Res<Arc<OwnedFd>> {
        match parent {
            Node::Root => config.src.clone().ok_or(Errno::EROFS),
            Node::Project(project) if config.is_overlay_name(name) => {
                config.overlay_parent(project).map(Arc::new)
            }
            Node::Project(name) => config
                .project_backing(name)
                .map(Arc::new)
                .ok_or(Errno::EROFS),
            other => self.existing_dir(config, other),
        }
    }

    fn check_fixed(config: &Config, parent: &Node, name: &OsStr) -> Res<()> {
        let fixed = match parent {
            Node::Project(_) => config.is_overlay_name(name),
            Node::Root => config.is_overlay_only_project(name),
            Node::Real(_) => false,
        };
        if fixed { Err(Errno::EPERM) } else { Ok(()) }
    }

    fn give_to_caller(config: &Config, dir: &OwnedFd, name: &OsStr, owner: Owner) -> Res<()> {
        if config.squash.is_some() {
            return Ok(());
        }
        let dir_st = rustix::fs::fstat(dir).map_err(sys)?;
        let gid =
            (dir_st.st_mode & libc::S_ISGID == 0).then(|| rustix::fs::Gid::from_raw(owner.gid));
        rustix::fs::chownat(
            dir,
            name,
            Some(rustix::fs::Uid::from_raw(owner.uid)),
            gid,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(sys)
    }

    fn created_entry(
        &self,
        config: &Config,
        parent: u64,
        parent_node: &Node,
        dir: &OwnedFd,
        name: &OsStr,
    ) -> Res<FileAttr> {
        let attr = if matches!(parent_node, Node::Root) && config.is_project(name)? {
            self.project_attr(config, name)?
        } else {
            self.real_entry(config, dir, name)?
        };
        self.record(parent, parent_node, name);
        Ok(attr)
    }

    fn make(
        &self,
        parent: u64,
        name: &OsStr,
        owner: Owner,
        create: impl FnOnce(&OwnedFd) -> rustix::io::Result<()>,
    ) -> Res<FileAttr> {
        let config = self.config();
        let parent_node = self.node(parent)?;
        let dir = self.creatable_dir(&config, &parent_node, name)?;
        create(&dir).map_err(sys)?;
        Self::give_to_caller(&config, &dir, name, owner)?;
        self.created_entry(&config, parent, &parent_node, &dir, name)
    }

    fn create(
        &self,
        parent: u64,
        name: &OsStr,
        mode: u32,
        umask: u32,
        flags: i32,
        owner: Owner,
    ) -> Res<(FileAttr, u64)> {
        let config = self.config();
        let parent_node = self.node(parent)?;
        let dir = self.creatable_dir(&config, &parent_node, name)?;
        let oflags = open_flags(flags) | OFlags::CREATE;
        let fd = rustix::fs::openat(
            &*dir,
            name,
            oflags,
            Mode::from_raw_mode(mode & !umask & 0o7777),
        )
        .map_err(sys)?;
        Self::give_to_caller(&config, &dir, name, owner)?;
        let attr = self.created_entry(&config, parent, &parent_node, &dir, name)?;
        let fh = self.add_handle(Handle::File(Arc::new(File::from(fd))));
        Ok((attr, fh))
    }

    fn link(&self, id: u64, new_parent: u64, new_name: &OsStr) -> Res<FileAttr> {
        let config = self.config();
        let Node::Real(source) = self.node(id)? else {
            return Err(Errno::EPERM);
        };
        let parent_node = self.node(new_parent)?;
        let dir = self.creatable_dir(&config, &parent_node, new_name)?;
        rustix::fs::linkat(&*source, "", &*dir, new_name, AtFlags::EMPTY_PATH).map_err(sys)?;
        self.created_entry(&config, new_parent, &parent_node, &dir, new_name)
    }

    fn remove(&self, parent: u64, name: &OsStr, flags: AtFlags) -> Res<()> {
        let config = self.config();
        let parent_node = self.node(parent)?;
        Self::check_fixed(&config, &parent_node, name)?;
        let dir = self.existing_dir(&config, &parent_node)?;
        rustix::fs::unlinkat(&*dir, name, flags).map_err(sys)
    }

    fn rename(
        &self,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
        flags: RenameFlags,
    ) -> Res<()> {
        let config = self.config();
        let parent_node = self.node(parent)?;
        let new_parent_node = self.node(new_parent)?;
        Self::check_fixed(&config, &parent_node, name)?;
        Self::check_fixed(&config, &new_parent_node, new_name)?;
        let dir = self.existing_dir(&config, &parent_node)?;
        let new_dir = self.creatable_dir(&config, &new_parent_node, new_name)?;
        let flags = rustix::fs::RenameFlags::from_bits_retain(flags.bits());
        rustix::fs::renameat_with(&*dir, name, &*new_dir, new_name, flags).map_err(sys)
    }

    #[allow(clippy::too_many_arguments)]
    fn setattr(
        &self,
        id: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        fh: Option<FileHandle>,
    ) -> Res<FileAttr> {
        let config = self.config();
        let fd = self.backing(&config, id)?;
        if let Some(mode) = mode {
            rustix::fs::chmod(proc_path(&fd).as_str(), Mode::from_raw_mode(mode & 0o7777))
                .map_err(sys)?;
        }
        if config.squash.is_none() && (uid.is_some() || gid.is_some()) {
            rustix::fs::chownat(
                &*fd,
                "",
                uid.map(rustix::fs::Uid::from_raw),
                gid.map(rustix::fs::Gid::from_raw),
                AtFlags::EMPTY_PATH,
            )
            .map_err(sys)?;
        }
        if let Some(size) = size {
            let file = match fh.and_then(|fh| self.file(fh.0).ok()) {
                Some(file) => file,
                None => Arc::new(OpenOptions::new().write(true).open(proc_path(&fd))?),
            };
            file.set_len(size)?;
        }
        if atime.is_some() || mtime.is_some() {
            let times = Timestamps {
                last_access: timespec(atime),
                last_modification: timespec(mtime),
            };
            rustix::fs::utimensat(
                rustix::fs::CWD,
                proc_path(&fd).as_str(),
                &times,
                AtFlags::empty(),
            )
            .map_err(sys)?;
        }
        self.getattr(id)
    }

    fn readlink(&self, id: u64) -> Res<Vec<u8>> {
        let Node::Real(fd) = self.node(id)? else {
            return Err(Errno::EINVAL);
        };
        let target = rustix::fs::readlinkat(&*fd, "", Vec::new()).map_err(sys)?;
        Ok(target.into_bytes())
    }

    fn open(&self, id: u64, flags: i32) -> Res<u64> {
        let Node::Real(fd) = self.node(id)? else {
            return Err(Errno::EISDIR);
        };
        let file = rustix::fs::open(proc_path(&fd).as_str(), open_flags(flags), Mode::empty())
            .map_err(sys)?;
        Ok(self.add_handle(Handle::File(Arc::new(File::from(file)))))
    }

    fn read(&self, fh: u64, offset: u64, size: u32) -> Res<Vec<u8>> {
        let file = self.file(fh)?;
        let mut buf = vec![0; size as usize];
        let n = file.read_at(&mut buf, offset)?;
        buf.truncate(n);
        Ok(buf)
    }

    fn write(&self, fh: u64, offset: u64, data: &[u8]) -> Res<u32> {
        let file = self.file(fh)?;
        file.write_all_at(data, offset)?;
        Ok(data.len() as u32)
    }

    fn opendir(&self, id: u64) -> Res<u64> {
        let config = self.config();
        let entries = match self.node(id)? {
            Node::Root => self.root_entries(&config)?,
            Node::Project(name) => self.project_entries(&config, id, &name)?,
            Node::Real(fd) => {
                let mut entries = vec![dot(".", id), dot("..", id)];
                entries.extend(self.dir_entries(&fd, |_| false)?);
                entries
            }
        };
        Ok(self.add_handle(Handle::Dir(Arc::new(entries))))
    }

    fn root_entries(&self, config: &Config) -> Res<Vec<DirEntry>> {
        let mut entries = vec![dot(".", ROOT_ID), dot("..", ROOT_ID)];
        let mut names = HashSet::new();
        let mut nodes = lock(&self.nodes);
        if let Some(src) = &config.src {
            let dev = device(src)?;
            for entry in list(src)? {
                if !names.insert(entry.name.clone()) {
                    continue;
                }
                let id = match entry.kind {
                    FileType::Directory => nodes.project_id(&entry.name),
                    _ => nodes.id_for_key((dev, entry.id)),
                };
                entries.push(DirEntry { id, ..entry });
            }
        }
        if let Some(overlay) = &config.overlay {
            for entry in list(&overlay.src)? {
                if entry.kind == FileType::Directory && !names.contains(&entry.name) {
                    let id = nodes.project_id(&entry.name);
                    entries.push(DirEntry { id, ..entry });
                }
            }
        }
        Ok(entries)
    }

    fn project_entries(&self, config: &Config, id: u64, project: &OsStr) -> Res<Vec<DirEntry>> {
        let mut entries = vec![dot(".", id), dot("..", ROOT_ID)];
        if let Some(backing) = config.project_backing(project) {
            entries.extend(self.dir_entries(&backing, |name| config.is_overlay_name(name))?);
        }
        if let Some(overlay) = &config.overlay {
            for name in &overlay.directories {
                if let Some(fd) = config.overlay_entry(project, name)? {
                    let st = rustix::fs::fstat(&fd).map_err(sys)?;
                    entries.push(DirEntry {
                        id: lock(&self.nodes).id_for_key(key(&st)),
                        kind: kind(st.st_mode),
                        name: name.clone(),
                    });
                }
            }
        }
        Ok(entries)
    }

    fn dir_entries(&self, fd: &OwnedFd, hidden: impl Fn(&OsStr) -> bool) -> Res<Vec<DirEntry>> {
        let dev = device(fd)?;
        let mut entries = list(fd)?;
        entries.retain(|entry| !hidden(&entry.name));
        let mut nodes = lock(&self.nodes);
        for entry in &mut entries {
            entry.id = nodes.id_for_key((dev, entry.id));
        }
        Ok(entries)
    }

    fn statfs(&self) -> Res<rustix::fs::StatVfs> {
        let config = self.config();
        let dir = match &config.src {
            Some(src) => src,
            None => config.overlay_root()?,
        };
        rustix::fs::fstatvfs(&**dir).map_err(sys)
    }

    fn xattr_target(&self, id: u64) -> Res<Arc<OwnedFd>> {
        self.backing(&self.config(), id)
    }

    fn add_handle(&self, handle: Handle) -> u64 {
        let mut handles = lock(&self.handles);
        handles.next += 1;
        let fh = handles.next;
        handles.map.insert(fh, handle);
        fh
    }

    fn file(&self, fh: u64) -> Res<Arc<File>> {
        match lock(&self.handles).map.get(&fh) {
            Some(Handle::File(file)) => Ok(file.clone()),
            _ => Err(Errno::EBADF),
        }
    }

    fn dir(&self, fh: u64) -> Res<Arc<Vec<DirEntry>>> {
        match lock(&self.handles).map.get(&fh) {
            Some(Handle::Dir(entries)) => Ok(entries.clone()),
            _ => Err(Errno::EBADF),
        }
    }

    fn release(&self, fh: u64) {
        lock(&self.handles).map.remove(&fh);
    }
}

/// Lists a directory. Each entry's `id` holds the raw inode number from the directory entry.
fn list(fd: &OwnedFd) -> Res<Vec<DirEntry>> {
    let dir_fd = rustix::fs::openat(
        fd,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(sys)?;
    let dir = rustix::fs::Dir::new(dir_fd).map_err(sys)?;
    let mut entries = vec![];
    for entry in dir {
        let entry = entry.map_err(sys)?;
        let name = OsStr::from_bytes(entry.file_name().to_bytes()).to_owned();
        if name == "." || name == ".." {
            continue;
        }
        let kind = match entry.file_type() {
            rustix::fs::FileType::Unknown => kind(
                rustix::fs::statat(fd, &name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(sys)?
                    .st_mode,
            ),
            other => kind(other.as_raw_mode()),
        };
        entries.push(DirEntry {
            id: entry.ino(),
            kind,
            name,
        });
    }
    Ok(entries)
}

fn dot(name: &str, id: u64) -> DirEntry {
    DirEntry {
        id,
        kind: FileType::Directory,
        name: name.into(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn sys(err: rustix::io::Errno) -> Errno {
    Errno::from_i32(err.raw_os_error())
}

fn ignore_exists(result: rustix::io::Result<()>) -> Res<()> {
    match result {
        Ok(()) | Err(rustix::io::Errno::EXIST) => Ok(()),
        Err(err) => Err(sys(err)),
    }
}

fn open_path(dir: impl AsFd, name: &OsStr) -> rustix::io::Result<OwnedFd> {
    rustix::fs::openat(
        dir,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

fn open_dir_path(dir: impl AsFd, name: &OsStr) -> rustix::io::Result<OwnedFd> {
    rustix::fs::openat(
        dir,
        name,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// `O_PATH` descriptors do not allow chmod, truncate, utimes, or xattr calls. These calls go
/// through the `/proc` link of the descriptor instead.
fn proc_path(fd: &OwnedFd) -> String {
    format!("/proc/self/fd/{}", fd.as_raw_fd())
}

fn open_flags(flags: i32) -> OFlags {
    OFlags::from_bits_retain(flags as u32) & !OFlags::NOCTTY | OFlags::CLOEXEC
}

fn timespec(time: Option<TimeOrNow>) -> Timespec {
    match time {
        None => Timespec {
            tv_sec: 0,
            tv_nsec: rustix::fs::UTIME_OMIT,
        },
        Some(TimeOrNow::Now) => Timespec {
            tv_sec: 0,
            tv_nsec: rustix::fs::UTIME_NOW,
        },
        Some(TimeOrNow::SpecificTime(time)) => {
            let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
            Timespec {
                tv_sec: since.as_secs() as i64,
                tv_nsec: since.subsec_nanos().into(),
            }
        }
    }
}

fn is_dir(st: &Stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFDIR
}

// Stat field types differ between architectures, so some casts are no-ops on x86_64.
#[allow(clippy::unnecessary_cast)]
fn key(st: &Stat) -> Key {
    (st.st_dev as u64, st.st_ino as u64)
}

fn device(fd: &OwnedFd) -> Res<u64> {
    Ok(key(&rustix::fs::fstat(fd).map_err(sys)?).0)
}

fn kind(mode: u32) -> FileType {
    match mode & libc::S_IFMT {
        libc::S_IFDIR => FileType::Directory,
        libc::S_IFLNK => FileType::Symlink,
        libc::S_IFIFO => FileType::NamedPipe,
        libc::S_IFSOCK => FileType::Socket,
        libc::S_IFCHR => FileType::CharDevice,
        libc::S_IFBLK => FileType::BlockDevice,
        _ => FileType::RegularFile,
    }
}

// Stat field types differ between architectures, so some casts are no-ops on x86_64.
#[allow(clippy::unnecessary_cast)]
fn attr(id: u64, st: &Stat, squash: Option<Squash>) -> FileAttr {
    let (uid, gid) = match squash {
        Some(squash) => (squash.uid, squash.gid),
        None => (st.st_uid as u32, st.st_gid as u32),
    };
    FileAttr {
        ino: INodeNo(id),
        size: st.st_size as u64,
        blocks: st.st_blocks as u64,
        atime: system_time(st.st_atime as i64, st.st_atime_nsec as u32),
        mtime: system_time(st.st_mtime as i64, st.st_mtime_nsec as u32),
        ctime: system_time(st.st_ctime as i64, st.st_ctime_nsec as u32),
        crtime: UNIX_EPOCH,
        kind: kind(st.st_mode as u32),
        perm: (st.st_mode & 0o7777) as u16,
        nlink: st.st_nlink as u32,
        uid,
        gid,
        rdev: st.st_rdev as u32,
        blksize: st.st_blksize as u32,
        flags: 0,
    }
}

fn system_time(secs: i64, nsecs: u32) -> SystemTime {
    let base = match u64::try_from(secs) {
        Ok(secs) => UNIX_EPOCH + Duration::from_secs(secs),
        Err(_) => UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs()),
    };
    base + Duration::from_nanos(nsecs.into())
}

pub struct Fs(pub Arc<View>);

fn reply_entry(reply: ReplyEntry, result: Res<FileAttr>) {
    match result {
        Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
        Err(err) => reply.error(err),
    }
}

fn reply_empty(reply: ReplyEmpty, result: Res<()>) {
    match result {
        Ok(()) => reply.ok(),
        Err(err) => reply.error(err),
    }
}

fn reply_xattr(reply: ReplyXattr, size: u32, result: rustix::io::Result<Vec<u8>>) {
    match result {
        Ok(data) if size == 0 => reply.size(data.len() as u32),
        Ok(data) if data.len() > size as usize => reply.error(Errno::from_i32(libc::ERANGE)),
        Ok(data) => reply.data(&data),
        Err(err) => reply.error(sys(err)),
    }
}

fn read_xattr(
    call: impl Fn(&mut [u8]) -> rustix::io::Result<usize>,
) -> rustix::io::Result<Vec<u8>> {
    let len = call(&mut [])?;
    let mut buf = vec![0; len];
    let len = call(&mut buf)?;
    buf.truncate(len);
    Ok(buf)
}

impl Filesystem for Fs {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        reply_entry(reply, self.0.lookup(parent.0, name));
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        lock(&self.0.nodes).forget(ino.0, nlookup);
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.0.getattr(ino.0) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(err) => reply.error(err),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        _kill_suid_gid: bool,
        reply: ReplyAttr,
    ) {
        match self
            .0
            .setattr(ino.0, mode, uid, gid, size, atime, mtime, fh)
        {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(err) => reply.error(err),
        }
    }

    fn readlink(&self, _req: &Request, ino: INodeNo, reply: ReplyData) {
        match self.0.readlink(ino.0) {
            Ok(target) => reply.data(&target),
            Err(err) => reply.error(err),
        }
    }

    fn mknod(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        rdev: u32,
        owner: Owner,
        reply: ReplyEntry,
    ) {
        let result = self.0.make(parent.0, name, owner, |dir| {
            rustix::fs::mknodat(
                dir,
                name,
                rustix::fs::FileType::from_raw_mode(mode),
                Mode::from_raw_mode(mode & !umask & 0o7777),
                rdev.into(),
            )
        });
        reply_entry(reply, result);
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        owner: Owner,
        reply: ReplyEntry,
    ) {
        let result = self.0.make(parent.0, name, owner, |dir| {
            rustix::fs::mkdirat(dir, name, Mode::from_raw_mode(mode & !umask & 0o7777))
        });
        reply_entry(reply, result);
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        reply_empty(reply, self.0.remove(parent.0, name, AtFlags::empty()));
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        reply_empty(reply, self.0.remove(parent.0, name, AtFlags::REMOVEDIR));
    }

    fn symlink(
        &self,
        _req: &Request,
        parent: INodeNo,
        link_name: &OsStr,
        target: &Path,
        owner: Owner,
        reply: ReplyEntry,
    ) {
        let result = self.0.make(parent.0, link_name, owner, |dir| {
            rustix::fs::symlinkat(target, dir, link_name)
        });
        reply_entry(reply, result);
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        flags: RenameFlags,
        _owner: Option<Owner>,
        reply: ReplyEmpty,
    ) {
        reply_empty(
            reply,
            self.0.rename(parent.0, name, newparent.0, newname, flags),
        );
    }

    fn link(
        &self,
        _req: &Request,
        ino: INodeNo,
        newparent: INodeNo,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        reply_entry(reply, self.0.link(ino.0, newparent.0, newname));
    }

    fn open(
        &self,
        _req: &Request,
        ino: INodeNo,
        flags: OpenFlags,
        _kill_suid_gid: bool,
        reply: ReplyOpen,
    ) {
        match self.0.open(ino.0, flags.0) {
            Ok(fh) => reply.opened(FileHandle(fh), FopenFlags::empty()),
            Err(err) => reply.error(err),
        }
    }

    fn read(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        match self.0.read(fh.0, offset, size) {
            Ok(data) => reply.data(&data),
            Err(err) => reply.error(err),
        }
    }

    fn write(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        match self.0.write(fh.0, offset, data) {
            Ok(written) => reply.written(written),
            Err(err) => reply.error(err),
        }
    }

    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _lock_owner: LockOwner,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.0.release(fh.0);
        reply.ok();
    }

    fn fsync(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    ) {
        let result = self.0.file(fh.0).and_then(|file| {
            let synced = if datasync {
                file.sync_data()
            } else {
                file.sync_all()
            };
            synced.map_err(Errno::from)
        });
        reply_empty(reply, result);
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        match self.0.opendir(ino.0) {
            Ok(fh) => reply.opened(FileHandle(fh), FopenFlags::empty()),
            Err(err) => reply.error(err),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let entries = match self.0.dir(fh.0) {
            Ok(entries) => entries,
            Err(err) => return reply.error(err),
        };
        for (index, entry) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(entry.id), index as u64 + 1, entry.kind, &entry.name) {
                break;
            }
        }
        reply.ok();
    }

    fn releasedir(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        reply: ReplyEmpty,
    ) {
        self.0.release(fh.0);
        reply.ok();
    }

    fn fsyncdir(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        match self.0.statfs() {
            Ok(st) => reply.statfs(
                st.f_blocks,
                st.f_bfree,
                st.f_bavail,
                st.f_files,
                st.f_ffree,
                st.f_bsize as u32,
                st.f_namemax as u32,
                st.f_frsize as u32,
            ),
            Err(err) => reply.error(err),
        }
    }

    fn setxattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        name: &OsStr,
        value: &[u8],
        flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        let result = self.0.xattr_target(ino.0).and_then(|fd| {
            let flags = rustix::fs::XattrFlags::from_bits_retain(flags as u32);
            rustix::fs::setxattr(proc_path(&fd).as_str(), name, value, flags).map_err(sys)
        });
        reply_empty(reply, result);
    }

    fn getxattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, size: u32, reply: ReplyXattr) {
        match self.0.xattr_target(ino.0) {
            Ok(fd) => {
                let path = proc_path(&fd);
                reply_xattr(
                    reply,
                    size,
                    read_xattr(|buf| rustix::fs::getxattr(path.as_str(), name, buf)),
                );
            }
            Err(_) => reply.error(Errno::NO_XATTR),
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        match self.0.xattr_target(ino.0) {
            Ok(fd) => {
                let path = proc_path(&fd);
                reply_xattr(
                    reply,
                    size,
                    read_xattr(|buf| rustix::fs::listxattr(path.as_str(), buf)),
                );
            }
            Err(_) => reply_xattr(reply, size, Ok(vec![])),
        }
    }

    fn removexattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let result = self
            .0
            .xattr_target(ino.0)
            .and_then(|fd| rustix::fs::removexattr(proc_path(&fd).as_str(), name).map_err(sys));
        reply_empty(reply, result);
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        flags: i32,
        _kill_suid_gid: bool,
        owner: Owner,
        reply: ReplyCreate,
    ) {
        match self.0.create(parent.0, name, mode, umask, flags, owner) {
            Ok((attr, fh)) => reply.created(
                &TTL,
                &attr,
                Generation(0),
                FileHandle(fh),
                FopenFlags::empty(),
            ),
            Err(err) => reply.error(err),
        }
    }

    fn fallocate(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        length: u64,
        mode: i32,
        reply: ReplyEmpty,
    ) {
        let result = self.0.file(fh.0).and_then(|file| {
            let flags = rustix::fs::FallocateFlags::from_bits_retain(mode as u32);
            rustix::fs::fallocate(&*file, flags, offset, length).map_err(sys)
        });
        reply_empty(reply, result);
    }
}
