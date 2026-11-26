# Design

## Context

See proposal.md for the motivation and specs/project-view/spec.md for the new requirements.

Current code:

- `src/main.rs` parses `--listen`, raises `RLIMIT_NOFILE`, and calls `api::serve`. `serve` creates an empty `Registry`.
- `Registry::mount` in `src/mounts.rs` checks params, reads `/proc/self/mountinfo` for foreign mounts, and starts a `fuser` session per `dst`. `parse_mountinfo` returns the mount point and file system type of each mount.
- `view::Config` holds `src: Arc<OwnedFd>`. The root node, the project list, root creates, `setattr` and xattrs on the root, and `statfs` use `src`.
- Projects without a `src` directory already follow the read-only rules. A restored view is that case for every project.
- The daemon mounts with `mount(2)` directly, without `auto_unmount`. A stopped daemon leaves a dead FUSE mount: `stat` on it fails with `ENOTCONN`, and mountinfo shows file system type `fuse` with source `vm-herd-fs`.

## Goals / Non-Goals

**Goals:**

- The IDE finds every project directory and its `.idea` right after a daemon start, before any `mount` call.
- A restored `dst` accepts a later `mount` in place, with stable inodes.

**Non-Goals:**

- Restore of `src`. A restored view shows overlay directories only.
- More than one daemon on a host.
- Cleanup of dead mounts during `mount` calls. A `dst` that the state file does not hold and that has a dead mount still fails as a foreign mount.
- A process manager or unit file for the daemon.

## Decisions

### Optional `src` in the view config

`Config.src` becomes `Option<Arc<OwnedFd>>`. With `None`:

- `is_project` checks only `overlay.src`. `project_backing` returns `None`, so every project follows the read-only rules.
- The root attributes come from `fstat` of `overlay.src` with mode `0755`, as for read-only projects.
- The root listing holds only the directories of `overlay.src`.
- Creates at the root return `EROFS`. `setattr` and xattr writes on the root return `EROFS`. `is_overlay_only_project` is true for every overlay project, so their removal and rename return `EPERM`.
- `statfs` uses `overlay.src`.

A `mount` call always sets `src`. Only the restore creates a config without `src`, and a restored config always has `overlay`.

Alternative: an empty directory in the data directory as `src`. Creates at the root would land in it, and the spec requires `EROFS`.

### State file types and writes

`src/state.rs` holds the file types:

```rust
struct State { version: u32, mounts: Vec<StoredMount> }
struct StoredMount { dst: PathBuf, overlay: OverlayParams, squash: Option<Squash> }
```

`OverlayParams` and `Squash` are the `mount` param types, so the items have the form of the `mount` params. `dst` is the resolved path, the same key as in the registry.

The registry keeps the items in memory, keyed by `dst`. After each successful `mount`, it sets or removes the item for that `dst` and writes the whole file. It writes `state.json.tmp` in the data directory, calls `fsync`, and renames the file over `state.json`. A rename in one directory replaces the file in one step, so a crash leaves either the old or the new file.

A failed state write after a successful `mount` returns `-32000` with the reason. The view stays mounted.

### Version check

The daemon first parses only `{"version": ...}`, then the full `State`. A later version can then change `mounts` without a parse error that hides the version. A version other than `1`, or a file that is not valid JSON of this form, stops the daemon with exit code 1 and a message.

### Start order

```
main
 |-- create --data-dir (create_dir_all)
 |-- load state.json ------ error --> print, exit 1
 |-- Registry::restore
 |     for each item:
 |       vm-herd-fs mount at dst? -------> umount2(dst, MNT_DETACH)
 |       dst missing / not a dir ------> skip, print reason
 |       mount at or below dst --------> skip, print reason
 |       overlay.src missing ----------> skip, print reason
 |       else mount view with src = None
 |-- api::serve(registry)
```

The API starts after the restore, so no `mount` call races with it.

### Detach of earlier mounts

`parse_mountinfo` also returns the mount source, the first field after the file system type. A mount at `dst` with type `fuse` and source `vm-herd-fs` comes from an earlier run, because only one daemon runs on a host. The daemon does not check whether it is alive. It calls `umount2` with `MNT_DETACH` through `rustix::mount::unmount`, which needs the `mount` feature of `rustix`.

The check reads mountinfo by the stored path and does not resolve `dst` first. Path resolution of a dead mount point fails with `ENOTCONN`.


### Shared mount code

`Registry::mount` and `Registry::restore` share one function that builds the `fuser` session for a `dst` and a config and inserts it into the registry. The foreign-mount check stays in both paths.

### Tests

The test daemon helper takes a data directory and can restart the daemon on the same directory. Its teardown detaches every `dst` that the test mounted or restored. The restart tests kill the daemon with `SIGKILL`, which leaves the dead mount that the restore must detach.

## Risks / Trade-offs

- [A restored view shows projects with only `.idea` until the caller sends `mount`, and the IDE may show the files as deleted] → This is the requested behavior. The caller sends `mount` after a daemon start.
- [A second daemon starts while the first one runs, and detaches its live views] → Out of scope: only one daemon runs on a host.
- [A stored `overlay.src` was moved] → The item is skipped and stays in the file, so the next start tries again.
