# Design

## Context

See proposal.md for the motivation and specs/project-view/spec.md for the required behavior. The repo has no code yet.

Facts that shape the design:

- The caller sends a full config per `dst` through JSON-RPC. It changes host by sending a new `src` for the same `dst`.
- JetBrains `fsnotifier` treats `fuse*` mounts (except `fuseblk`) as unwatchable (`native/fsNotifier/linux/main.c`, `is_watchable`). The IDE refreshes such roots without inotify.
- FUSE protocol 7.45 has no message that creates inotify events. The user accepted the loss of inotify.
- The host runs only the IDE on the view. No builds or other long tasks run there during a config change.

## Goals / Non-Goals

**Goals:**

- One daemon owns each view. A config change replaces an in-memory config, not a mount.
- Ownership squash that matches the current bindfs call, independent of how other writers create files.

**Non-Goals:**

- inotify events for changes under `dst`.
- Daemon restart and config persistence, API authentication, and an unmount method.
- Checks for processes that use a project during a config change.
- Kernel FUSE passthrough (`FUSE_PASSTHROUGH`) and multi-threaded request handling. Both are possible later optimizations.

## Decisions

### Rust with the `fuser` crate, low-level API

The daemon uses the `fuser` low-level `Filesystem` trait, so it controls every node ID. `rustix` gives the `*at` system calls. `tiny_http` serves the API, and `serde_json` handles JSON-RPC. `clap` parses `--listen <host:port>`.

Alternatives:
- Python with `pyfuse3`: slower per request, and every IDE `stat` goes through the daemon.
- `axum` on `tokio`: an async runtime for one endpoint that handles one call at a time.

### Node table keyed by the backing inode

Each real node holds an `O_PATH` file descriptor of its backing object. The key is the backing `(st_dev, st_ino)`. `lookup` opens the child with `openat(parent_fd, name, O_PATH | O_NOFOLLOW)` and reuses the node when the key exists. `forget` closes the descriptor when the kernel lookup count reaches 0. This is the model of libfuse `passthrough_hp` and virtiofsd.

A handle follows a file that another writer renames, so open files keep their content. A path-based model breaks on such renames.

Node IDs are never reused while the daemon runs. The reported `st_ino` is the node ID. Thus inode numbers stay stable, and git does not rehash files after a `forget`. The key-to-ID map grows with the number of distinct files seen, about 50 bytes per file.

Operations on real nodes use the descriptor: `fstatat` with `AT_EMPTY_PATH`, and `/proc/self/fd/<n>` for xattrs and `open`. At start, the daemon raises `RLIMIT_NOFILE` to the hard limit.

### Virtual root and project nodes

Each mount has a virtual root (node 1) and one virtual node per project name. A project is a directory at the root of `src` or `overlay.src`. Project nodes are created on first lookup and are never reused, so a project keeps its inode number across config changes.

- Root `readdir` lists the entries of `src` plus the directories of `overlay.src`. Files at the root of `src` are real nodes. Create, rename, and remove calls in the root act on `src`.
- A project node delegates to `src/<p>` when that directory exists. Otherwise it is read-only: only the overlay directories, `EROFS` for creates, and `EPERM` for its own removal or rename. It reports mode `0755`. With `0555`, the kernel rejects a non-root create with `EACCES` before the daemon can return `EROFS`.
- A lookup of a name in `overlay.directories` under a project node returns the real node of `overlay.src/<p>/<name>`, or `ENOENT` when it does not exist. A create of such a name goes to `overlay.src/<p>`, which the daemon creates when it is missing. This works in read-only projects too. `readdir` of a project drops these names from `src` and adds the overlay entries that exist. Removal or rename of these entries returns `EPERM`.

When a rename in the root moves `src/proj1` to `src/projX`, the overlay directories stay at `overlay.src/proj1`. `projX` has no overlay directories until one is created.

### Mount and in-place config change

The daemon keeps a map from resolved `dst` to a mount entry. Each entry has a `fuser` background session and a `RwLock` around its config. `mount` runs these steps:

1. Check the params and resolve the paths. Reject path overlaps with `-32602`.
2. If `dst` has no entry, read `/proc/self/mountinfo`. If `dst` or a path below it is a mount point, return `-32000` with the mount point and file system type. Otherwise mount with `allow_other`, `default_permissions`, `nosuid`, `nodev`, and `fsname=vm-herd-fs`.
3. If `dst` has an entry, take the write lock, set the new config, and release the lock.
4. Send `inval_entry` for each name that the kernel has looked up under the root and under each project node. The daemon records these names in `lookup`. Send `inval_inode` for the root and each project node. When `squash` changes, also send `inval_inode` for every node in the table.

The kernel holds a directory lock while it waits for a reply. A notification sent from a FUSE request handler can therefore wait on its own thread. The API thread sends all notifications, and it never holds the config lock while it does so.

These notifications only mark cache entries stale. Open files and working directories do not block them, so a config change never fails with `EBUSY`. A process with its working directory in a project node sees the new `src` at once. A process with its working directory deeper keeps the old directory until it looks the path up again.

The daemon runs as root, so `fuser` mounts with `mount(2)` directly and does not run `fusermount3`. The mount does not use `auto_unmount`. That option needs a `fusermount3` watcher, and the Ubuntu 26.04 AppArmor profile for `fusermount3` blocks the mount namespace that the watcher uses. When the daemon ends, each `dst` stays as a dead mount. A later `mount` call for it reports a foreign mount until the user runs `umount -l`.

### Cache timeouts for external changes

Entry and attribute replies use a 1-second TTL. Negative lookups use a TTL of 0, so a new project shows at once. `open` does not set `FOPEN_KEEP_CACHE`, so the kernel drops cached pages at each open. Together these give the 1-second visibility of changes from other writers. The cost is more requests from the kernel to the daemon.

### Ownership and permissions

The kernel checks permissions against the attributes that the daemon reports (`default_permissions`). The daemon runs as root and does each operation as root.

- With `squash`, each attribute reply sets the owner to `squash.uid`:`squash.gid`. New entries get on-disk owner 0:0. `setattr` ignores `uid` and `gid`. The kernel still rejects a chown by a non-root user to another ID with `EPERM`, before the daemon sees it.
- Without `squash`, attribute replies show the on-disk owner. After each create, the daemon runs `fchownat` with the caller's UID and GID from the request header. `setattr` applies `uid` and `gid` with `fchownat`.

In both cases the daemon applies the request `umask` to new modes.

### API

`tiny_http` serves `POST /api` on the `--listen` address. It handles one request at a time, so `mount` calls never interleave. The JSON-RPC method `mount` lives in `api.rs`. Other paths return HTTP 404, and other HTTP methods return HTTP 405. JSON-RPC errors use the standard codes `-32700`, `-32600`, `-32601`, and `-32602`. Mount failures, including foreign mounts, use `-32000` with a message.

### Project layout and checks

The project is a Cargo crate `vm-herd-fs` with one binary. A `Dockerfile` (Rust slim) and `compose.yaml` hold a `check` profile and a `test` profile. `check` runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`. `test` runs `cargo test` in a container with `/dev/fuse` and `SYS_ADMIN`. The integration tests start the daemon on a free local port, call `mount` with `ureq`, and act as UID 1000 through `setpriv`.

## Risks / Trade-offs

- [The IDE does not refresh after a config change or after NFS writes while it has focus] → The user runs "Reload All from Disk". No test covers IDE behavior.
- [Slow IDE refresh and `git status` on large trees, because every request goes through one thread] → Performance is out of scope. Kernel passthrough and more threads are later options.
- [A daemon crash leaves each view as a dead mount, and the IDE reports missing projects] → No `panic` paths in request handlers: each error becomes an errno. Restart is out of scope. The user runs `umount -l` before the next `mount` call.
- [The API is open to every process that reaches the port] → Out of scope. The README tells the user to bind to `127.0.0.1`.
- [A process creates a setuid file through the view, which is a setuid-root file in `src` with `squash`] → The view is `nosuid`. Mount `src` with `nosuid` on the server and on NFS clients.
- [Moves and hard links between `src` and `overlay.src` fail with `EXDEV` when they are on different file systems] → `mv` and the IDE fall back to copy and delete.

## Migration Plan

1. Build the binary in the container and install it.
2. Start `vm-herd-fs --listen 127.0.0.1:<port>` as root.
3. Unmount the old bindfs mounts at `dst`. The daemon refuses a `dst` with a foreign mount.
4. Send `mount` from the caller.
5. In the IDE, dismiss the "External file changes sync may be slow" notice.

Rollback: stop the daemon, then run `umount -l` for each `dst`.
