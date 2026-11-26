# Proposal

## Why

Every view today assumes a disposable host: the daemon keeps IDE directories in an overlay and
restores only that overlay after a reboot. A persistent host needs the opposite. Its directory
must come back at the same path after a reboot, with no overlay in between.

## What Changes

- **BREAKING** `mount` gains a required param `type` with the values `cattle` and `pet`. A
  missing or other value fails with `-32602`.
- `cattle` is today's behavior, unchanged.
- `pet` shows `src` at `dst` as a plain passthrough with `squash`. `overlay` must be absent or
  `null`; otherwise `mount` fails with `-32602`.
- Every successful `pet` mount is stored in `state.json`. At start, the daemon mounts the stored
  `src` at `dst` again. A pet whose `src` is missing at start is skipped with a printed reason,
  and its item stays in the file.
- `umount` for a pet `dst` fails with `-32000` and changes nothing.
- **BREAKING** `state.json` items carry `type`. A cattle item holds `dst`, `overlay`, and
  `squash`. A pet item holds `dst`, `src`, and `squash`. `version` stays `1`.
- `mounts` items carry `type`. A restored pet shows its real `src`, not `null`.
- A `mount` of the other type on a served `dst` replaces the view in place, like any config
  change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: the `mount` params gain `type`; a new requirement defines the pet view and
  its param rules; state file format and updates, restore, mount points at start, umount, and
  `mounts` items cover pet items.

## Impact

- `src/api.rs`: `MountType` enum; `type` on `MountParams` and `MountsItem`.
- `src/state.rs`: `StoredMount` becomes a `type`-tagged enum with cattle and pet variants.
- `src/mounts.rs`: pet branch in `mount`, `umount`, `restore_item`, and the restored config.
- `src/view.rs`: no change. A pet config is `src` with no overlay stores.
- `tests/`: `type` in every mount literal; new tests for pet mount, store, restore, skip, and
  umount.
- `README.md`: param table, examples, umount and restart sections.
