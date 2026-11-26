# Design

## Context

See proposal.md for the motivation and specs/project-view/spec.md for the new requirements.

Current code:

- `api.rs` dispatches JSON-RPC methods. Only `mount` exists. Params are typed structs with `deny_unknown_fields`.
- `Registry` in `mounts.rs` holds `mounts: HashMap<PathBuf, Mounted>` keyed by the resolved `dst`, and `stored: BTreeMap<PathBuf, StoredMount>` that mirrors `state.json`.
- `Registry::restore_item` detaches an earlier mount, checks `dst`, builds a `Config` with `src: None` from a `StoredMount`, and mounts it.
- A served `dst` takes a new config in place through `View::replace` and the cache invalidation in `invalidate`.
- Dropping a `fuser` `BackgroundSession` unmounts the view lazily (`MNT_DETACH`). It then waits up to 5 seconds (`UNMOUNT_WAIT` in fuser 0.19) for the connection to end, which takes that long when a process still holds a file open.

## Goals / Non-Goals

**Goals:**

- After `umount`, `dst` is in the state a reboot and a daemon start produce.
- A `dst` with a stored overlay keeps its mount, so the IDE never loses a project or `.idea` path.

**Non-Goals:**

- Changes to `state.json`. A later `mount` without `overlay` still removes an item.
- Checks for processes that use the view.

## Decisions

### Restored config shared with the start restore

A new function `restored_config(item: &StoredMount) -> Result<Config, MountError>` holds the part of `restore_item` that checks the overlay names and opens `overlay.src`. `restore_item` and `umount` both call it, so the view after `umount` is exactly the view a start restores.

### `umount` flow

```
umount(dst)
 |-- dst not absolute -------------------> -32602
 |-- resolve dst; fails or not served ----> -32000 "dst is not served"
 |-- stored item for dst?
 |     yes --> restored_config(item) --- error --> -32000, view unchanged
 |             view.replace(config) + invalidate        (mount stays)
 |     no  --> umount2(dst, MNT_DETACH)                 (dst is plain now)
 |             remove the registry entry
 |             drop the session on a new thread
 '-- result null
```

The served check resolves `dst` with `canonicalize`, the same as `mount`. A served `dst` resolves through its live view.

### Unmount without waiting

`umount` detaches the mount itself with `rustix::mount::unmount(dst, DETACH)` before it drops the session. The reply therefore comes after `dst` is a plain directory. The session then goes to a new thread for its drop, so the up to 5 seconds that `fuser` waits for open files do not block the API. Processes that hold files open keep them until they close them, which `MNT_DETACH` allows.

Alternative: drop the session on the API thread. Each `umount` with an open file in the view then takes 5 seconds, and the single-threaded API serves no other call meanwhile.

### Params and errors

`UmountParams { dst: PathBuf }` lives in `api.rs` with `deny_unknown_fields`, next to `MountParams`. A relative `dst` returns `-32602`. A `dst` that is not served, a failure to open `overlay.src`, and a failed detach return `-32000`.

## Risks / Trade-offs

- [A stored `overlay.src` was removed, so `umount` cannot build the restored view] → The call fails with `-32000` and the current view stays. A daemon start would skip this item in the same case.
- [`umount` of a view without a stored overlay hides files that the IDE has open] → This is the post-reboot state the caller asks for.
