# Proposal

## Why

Each host keeps its projects in its own directory, for example `workdir/192.168.2.130/<project>`. The IDE opens one stable tree, for example `~/Dev/truenas`. No current tool shows one host's projects in that tree, keeps `.idea` in a separate store, and maps ownership like the current bindfs call. A change of host must never make a project or its `.idea` disappear, because the IDE then breaks.

## What Changes

- Add a Rust daemon `vm-herd-fs` with a FUSE file system based on the `fuser` crate.
- The daemon listens on a given `host:port` and accepts JSON-RPC 2.0 requests at `POST /api`.
- Add the method `mount` with the params `src`, `overlay` (nullable: `src`, `directories`), `dst`, and `squash` (nullable: `uid`, `gid`).
- A `mount` on a new `dst` mounts the view there. A `mount` on a `dst` that the daemon already serves replaces the config in place, without an unmount. A `mount` on a `dst` with another mount fails.
- The view at `dst` is a passthrough of `src`. It also lists every project directory of `overlay.src`, so projects of other hosts stay as read-only directories.
- In each project, the names in `overlay.directories` (for example `.idea`) come from `overlay.src/<project>/<name>`. The same names in `src` stay hidden and untouched.
- With `squash`, ownership follows the current bindfs call: every file shows as `uid:gid`, new files are stored as 0:0, and chown changes nothing on disk.
- inotify is out of scope. The IDE sees changes from other writers when it refreshes, for example when its window gets focus.

## Capabilities

### New Capabilities

- `project-view`: the `mount` API, the view composition, the overlay directories, ownership squash, and in-place config replacement.

### Modified Capabilities

None.

## Impact

- New Rust project `vm-herd-fs` with one binary. The repo has no code yet.
- The daemon runs as root and needs `/dev/fuse`. It mounts directly, without `fusermount3`.
- When the daemon stops or crashes, each `dst` stays as a dead mount until the user runs `umount -l`.
- JetBrains IDEs do not watch FUSE mounts. They show "External file changes sync may be slow" and refresh on window focus or "Reload All from Disk".
- Each `stat`, `open`, and directory listing under `dst` goes through the daemon. Large trees refresh slower than on a local disk.
- Out of scope: daemon restart and config persistence, API authentication, an unmount method, checks for processes that use a project during a config change, and performance.
