# Design

## Context

See proposal.md for the motivation. The working tree holds the uncommitted code of the change
`bind-shared-entries-into-src`. In it, `Mounted` owns a `Vec<Bind>`, `mount` and `umount` add
and remove bind mounts, and the start runs `detach_earlier_binds`, which matches earlier bind
mounts by device and inode. The functions `shared_binds`, `add_binds`, `add_bind`, `detach`,
and `create_owned` already do the checks, the mount points, the path-based `open_tree` and
`move_mount` calls, and the lazy detach with the removal of empty mount points.

`view.rs` still holds the shared store: `Config.shared`, the lookups and listing that merge
it, and the `EPERM` rules. `Config.individual` holds the IDE-directory store.

## Goals / Non-Goals

**Goals:**
- Bind mounts keyed by `src`, with no link to any view.
- One code path for `bind` and for the binds at start, so a restart never doubles a bind mount.
- No trace of the shared store in the view, the API, the state file, the tests, or the README.

**Non-Goals:**
- A method that lists the bind mounts.
- A check that `bind` and `mount` paths do not overlap each other.
- A read of the old `state.json` shape. The user removes the file once.

## Decisions

### `Registry` keeps a map from `src` to its bind mounts

`Registry` gets `binds: BTreeMap<PathBuf, Vec<Bind>>`, keyed by the resolved `src`, for the
bind mounts that this run made. `stored_binds: BTreeMap<PathBuf, PathBuf>` maps each stored
`src` to its store and is the source of `binds` in `state.json`, also for items that failed at
start. Each `Bind` already holds its store path. `Mounted` loses its `binds`, and `mount` and `umount`
lose every bind call. The existing `Bind { point, entry, dir }` stays.

### `bind` makes `src` match the store, with a presence check by inode

`bind` resolves `src` and `shared` with `existing_dir` (`-32602`), then runs `shared_binds` for
every check (`-32000`) before any change. A wanted bind is present when `src/<e>` and
`shared/<e>` have the same `(st_dev, st_ino)`, read without following a symlink. The daemon then:

1. Detaches each known bind of `src` whose point is wanted but whose entry differs, so a new
   bind mount never stacks on an old one.
2. Adds each wanted bind that is not present, with `add_binds`. A failure removes the binds
   that this step added.
3. Detaches each known bind whose point is not wanted.
4. Stores `Bound` and writes `state.json`.

The known binds come from the map, so step 3 sees only what this run bound. The inode check
lets the binds at start keep the bind mounts that a crashed run left.

Alternative: the device-and-inode scan of all of `/proc/self/mountinfo` from the old change.
Rejected: `state.json` now stores each `src`, so the daemon looks only at the paths it needs.

### `unbind` uses the map

`unbind` checks that `src` is absolute (`-32602`) and resolves it. A deleted `src` does not
resolve, so the daemon then uses the path as given. It fails with `-32000` when neither map
has the path, so a stored item whose bind failed at start can still go. It runs `detach` on each bind, removes the entry, and writes
`state.json`. `detach` already removes an empty mount point after the lazy detach.

### `state.json` gets `binds`, and the pet item loses `overlay`

`State` becomes `{version, mounts, binds}` with `binds: Vec<StoredBind { src, shared }>`,
sorted by `src`. Both arrays are required. `StoredMount::Pet` loses `overlay`.
`OverlayParams` becomes `{src, directories}` with `deny_unknown_fields`, so an old
`{"individual": ...}` shape fails with a clear serde message. `state::save` takes both lists.

### Binds before views at start

`Registry::restore` first runs `bind` logic for each stored item, with a print and a skip on
failure, then restores the views. `detach_earlier_binds` goes away. A pet view then starts
with its shared entries already in `src`.

### The view loses the shared store, and `Individual` becomes `Overlay`

`view.rs` drops `Shared`, `Config.shared`, `shared_entry`, `shared_has`, the merge in the root
listing, and the `EPERM` checks for shared names. `Individual` and `Config.individual` become
`Overlay` and `Config.overlay`. In `mounts.rs`, `open_shared` and `open_individual` go away or
get the new name. The project's rule for removed concepts asks for clean names everywhere.

### `bind` and `unbind` live in `api.rs`

`api.rs` dispatches each JSON-RPC method. It gets `BindParams { src, shared }` and
`UnbindParams { src }` with `deny_unknown_fields`, and calls `Registry::bind` and
`Registry::unbind`.

## Risks / Trade-offs

- [A tool deletes `src` with `rm -rf` before `unbind`] → `rm` goes through a bound directory
  and deletes files in the store. vm-herd must call `unbind` first. The README says so.
- [A store entry goes away while the daemon is down] → At start, the daemon does not know the
  old bind mount at `src/<e>`, so it stays until a reboot. It shows the deleted entry, and a
  `rm -rf` of `src` stops with `EBUSY` on it. No data goes away.
- [`src/<e>` is a hardlink of `shared/<e>`] → The inode check counts it as present and adds no
  bind mount. Both paths are the same file, so NFS clients see the same content.
- [A restored cattle view loses `notes.md` and `tools`] → The view has no `src`. An IDE tab on
  such a file can close until the next `mount`. The user accepted this.
- [`nfsd` holds a bind mount] → Every removal uses `DETACH`, as before.
