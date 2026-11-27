# Design

## Context

The working tree holds an uncommitted experiment. `Config.fallback_git` holds the hardcoded
path `/home/themylogin/test/.git`, and `mounts.rs` sets it for each cattle view. The view
opens that path on each lookup and listing. `Nodes` keys real nodes by `(realm, dev, ino)`.
The realm is 0 for normal entries, and the project node id for the substitute tree. Project
node ids are keyed by name and never change, so the realm stays stable across config swaps.
The substitute tree is writable today.

## Goals / Non-Goals

**Goals:**

- One store per view with any number of entries, passed and stored like `overlay`.
- Read-only access that the view enforces itself, on every kernel version that runs the daemon
  today.

**Non-Goals:**

- A fallback for pet views or at the root of a cattle view.
- A store per project.
- A change to `vm-herd`. It sends `fallback` once this change is out.

## Decisions

### The store is an open directory descriptor

`Config.fallback` becomes `Option<Arc<OwnedFd>>`. The daemon opens it at `mount` and at
restore, as it opens `overlay.src`. A store that goes missing later shows no entries. It does
not break the view. At start, a missing store skips the item, through the same path as a
missing `overlay.src`.

The alternative was a path that the view opens on each call, as in the experiment. It hides a
wrong path until the first lookup, and it gives no common place for the start check.

### Fallback entries are a lookup step after the overlay

For a `Node::Project` parent, the lookup order is: overlay name, then `src/<p>/<e>`, then
`fallback.src/<e>`. The view remembers an entry from the store with the realm of the project
node id. All lookups below it inherit that realm through `Node::Real(fd, realm)`. A listing of
a project adds each store entry whose name is not an overlay name and not in `src/<p>`. The
experiment's `.git`-only branch becomes this general step.

### The view enforces read-only access by realm

Each mutating call gets a check: a node or a parent with a realm other than 0 gives `EROFS`.
This covers `setattr`, `open` with `O_WRONLY`, `O_RDWR`, or `O_TRUNC`, `create`, `mknod`,
`mkdir`, `symlink`, `link` (source or new parent), `unlink`, `rmdir`, `rename` (either parent),
`setxattr`, and `removexattr`. `write` and `fallocate` need no check, because the file handle
is read-only. Removal or rename of a top-level fallback entry, with the project as parent,
also gives `EROFS`, next to the existing `EPERM` check for overlay names.

The alternative was a detached read-only clone of the store, made with `open_tree` and
`mount_setattr(MOUNT_ATTR_RDONLY)`. The kernel would then refuse every write by itself. But
`mount_setattr` needs Linux 5.12, and the kernel of the desktop host is unknown. A move or hard
link out of the store would also fail with `EXDEV` instead of `EROFS`. The realm check is a
few lines in one file, and it needs no kernel feature.

### Params and state follow the overlay pattern

- `api.rs` gets `FallbackParams { src }` with `deny_unknown_fields`.
- `MountParams`, `MountsItem`, `Shown`, and `StoredMount::Cattle` get
  `fallback: Option<FallbackParams>`.
- serde reads a missing `Option` as `None`, so an old cattle item loads with no other change,
  and `version` stays `1`.
- The stored item always writes `"fallback": null` or the object, as pet items do with
  `overlay`.

### Inode numbers stay with the project

The realm is the project node id, so an in-place swap that keeps the store keeps the same keys
and the same inode numbers. The existing invalidation of cached names under project nodes
already covers a swap that adds or removes the store.

## Risks / Trade-offs

- [Reads change `atime` in the store.] → No mitigation. The store holds placeholders.
- [Git commands in a project without its own `.git` fail to take `index.lock`.] → `git status`
  ignores this. The IDE keeps the Git root, which is the goal. Writes go to the real `.git`
  once `src` has one.
- [All projects of a view show the same repository.] → It is read-only, so no project can
  change it for the others.
- [Restored views lose the hardcoded `.git` until `vm-herd` mounts with `fallback`.] → Mount
  each `dst` once with `fallback` after the upgrade.

## Migration Plan

1. Build and restart the daemon. Existing `state.json` files load, with `fallback` `null`.
2. Create the store, for example `git init` in `<workdir>/truenas-fallback`, so that it holds
   `.git`.
3. Send `mount` with `fallback` for each cattle `dst`. The item in `state.json` then holds the
   store for later restores.

Rollback: an older binary rejects a cattle item with `fallback`, because of
`deny_unknown_fields`. Remove the key from `state.json` before you start it.
