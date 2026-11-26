# Proposal

## Why

The overlay store holds only per-project IDE directories. Files that belong to the view as a
whole, such as notes or scripts at the root of `dst`, have no place that survives a change of
host or a reboot. The daemon is not deployed yet, so the API can change shape now at no cost.

## What Changes

- **BREAKING** The `overlay` param of `mount` becomes an object with two members:
  `individual` and `shared`. Each member is `null` or an object. `overlay` and each member
  default to `null` when the key is absent.
- `overlay.individual` is the store that `overlay` is today: `{"src", "directories"}` with
  per-project directories.
- `overlay.shared` is new: `{"src"}`. Every entry of `shared.src`, file or directory, shows
  at the root of `dst` as a writable passthrough.
- A shared entry shadows a `src` entry of the same name. Project directories count. `src`
  stays untouched.
- Remove or rename of a shared entry at the root of `dst` fails with `EPERM`. Entries nested
  in a shared directory follow the normal passthrough rules.
- **BREAKING** `state.json` keeps `version` `1` and stores `overlay` in the new shape. A file
  in the old shape fails to parse, and the daemon exits with the reason.
- A restored view shows shared entries next to the projects of `overlay.individual.src`.
- `mounts` echoes `overlay` in the new shape.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: `overlay` splits into `overlay.individual` and `overlay.shared`. Param
  checks, project list, overlay directories, fixed entries, state file, restore, and
  `mounts` items follow the new shape. A new requirement defines shared entries.

## Impact

- `src/api.rs`: `OverlayParams` becomes `{individual, shared}`.
- `src/view.rs`: `Config` holds `individual` and `shared`; root lookup, listing, stat,
  statfs, and fixed-entry checks read `shared`.
- `src/mounts.rs`: param checks and config construction for `shared.src`.
- `src/state.rs`: `StoredMount.overlay` takes the new shape.
- `tests/`: every overlay JSON literal, plus new tests for shared entries.
- `README.md`: param table, examples, and the restart section.
