# Proposal

## Why

`mount` fails when `dst` does not exist, so every caller must create the mount point first and
set its owner by hand. The daemon runs as root and already knows the owner from `squash`. It can
do both steps itself.

## What Changes

- `mount` creates `dst` when it does not exist, as a directory with mode `0755`, after all param
  checks pass.
- With `squash`, the new directory gets on-disk owner `squash.uid:squash.gid`. Without `squash`,
  it keeps the daemon's owner, 0:0.
- An existing `dst` is not changed.
- `dst` must be an absolute path. When it exists, it must be a directory. When it is missing, its
  parent must be an existing directory; otherwise `mount` fails with `-32602`.
- A failed param check never creates `dst`. A failed create fails with `-32000`.
- Restore at start does not change: a missing `dst` still skips the item.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: the param check for `dst` allows a missing directory with an existing parent;
  a new requirement defines the create and its owner.

## Impact

- `src/mounts.rs`: `dst` resolution for a missing path, a create step before `attach`.
- `tests/api.rs`: tests for the create, the owner, an existing `dst`, a missing parent, and a
  failed check.
- `README.md`: the `dst` row and the paths paragraph.
