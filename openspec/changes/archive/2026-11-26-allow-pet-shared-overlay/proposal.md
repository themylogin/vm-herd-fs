# Proposal

## Why

A pet view shows one persistent host and nothing else. The user also wants shared entries, such
as notes and tools, at the root of a pet view. The daemon rejects `overlay` for a pet today, so
the only way to get shared entries is a cattle view, which loses the pet's restore behavior.

## What Changes

- `mount` with `type` `pet` accepts `overlay.shared`. `overlay.individual` must stay absent or
  `null`; otherwise `mount` fails with `-32602`. A pet has no projects, so IDE directories have
  no place in it.
- A pet view shows the entries of `overlay.shared.src` at its root, with the same rules as a
  cattle view: writable, fixed, and hiding a `src` entry of the same name.
- A pet item in `state.json` holds `overlay`. A pet mounted without `overlay` stores
  `"overlay": null`. A pet item without the key loads as `"overlay": null`, so an existing
  `state.json` keeps working. `version` stays `1`.
- At start, the restored pet view shows the stored `src` and the stored shared entries.
- A `mounts` item for a pet holds its `overlay`, with `individual` `null`, instead of `null`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: the requirements Mount type, Pet view, State file format, Restore at start,
  and Mounts items change to allow and carry `overlay.shared` on a pet.

## Impact

- `src/mounts.rs`: the pet check in `mount` narrows to `overlay.individual`; `restored_config`
  and `restored_shown` open and show the pet's shared store; the stored pet item gets `overlay`.
- `src/state.rs`: `StoredMount::Pet` gains `overlay: Option<OverlayParams>`.
- `src/view.rs` and `src/api.rs`: no change. `Config` already holds `src` and `shared`
  together, and the wire types already carry `overlay`.
- `tests/`: the pet-with-overlay test changes to `overlay.individual`; new tests for a pet with
  shared entries, its state item, its restore, and its `mounts` item.
- `README.md`: the `type` and `overlay` rows, the pet example, and the `state.json` example.
