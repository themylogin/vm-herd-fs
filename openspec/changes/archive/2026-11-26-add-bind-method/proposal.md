# Proposal

## Why

Every cattle VM and every pet reads its own `src` over NFS, and each one needs the shared
entries, even when the view at `dst` shows another host. Bind mounts that follow the view reach
only the host that the view shows. The shared entries must live in each host directory on their
own, so the view no longer needs a shared store.

## What Changes

- New JSON-RPC method `bind` with the params `src` and `shared`. It bind-mounts each file and
  directory of `shared` at `src/<entry>`, independent of any view. A later `bind` for the same
  `src` adds new store entries and removes the bind mounts of entries that are gone. It never
  stacks a second bind mount on a path.
- New JSON-RPC method `unbind` with the param `src`. It removes the bind mounts of `src` and the
  empty mount points that they leave. A tool must call it before it deletes `src`, because
  `rm -rf` goes through a bound directory into the store.
- `state.json` gets a `binds` array. At start, the daemon binds each stored pair again before
  it restores the views.
- **BREAKING** `overlay` becomes `{"src", "directories"}`, the former `overlay.individual`.
  `overlay.shared` goes away, together with the shared entries of the view and the `EPERM` rule
  for them. The view shows bound entries through the plain passthrough of `src`. A remove or a
  rename of a top-level bound entry fails with `EBUSY` from the kernel.
- **BREAKING** A pet takes no `overlay` again. A pet item in `state.json` holds `dst`, `src`, and
  `squash`.
- A restored cattle view shows only the projects of `overlay.src`, because it has no `src`.
- `version` of `state.json` stays `1`. An existing file has the old shape, and the user removes
  it once.
- This change replaces the unarchived change `bind-shared-entries-into-src`. Its uncommitted
  code moves the bind mounts with the view, and the tasks here rework it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: new requirements for `bind`, `unbind`, mount points, the `binds` in
  `state.json`, and binds at start; the flat `overlay` in the API, param checks, project list,
  overlay directories, state file, restore, `umount`, `mounts`, mount type, and pet view; the
  requirements Shared entries and Fixed shared entries are removed.

## Impact

- `src/api.rs`: the methods `bind` and `unbind`; `OverlayParams` becomes `{src, directories}`;
  `IndividualParams` and `SharedParams` go away.
- `src/view.rs`: the shared store, its lookups, its listing, and its `EPERM` checks go away;
  `Individual` becomes `Overlay`.
- `src/mounts.rs`: bind mounts move from `Mounted` to a per-`src` map in `Registry`; `mount`,
  `umount`, and restore stop binding.
- `src/state.rs`: the `binds` array, the flat `overlay`, and a pet item without `overlay`.
- `tests/`: `tests/binds.rs` moves to `bind` and `unbind`; the shared-entry tests in
  `tests/view.rs`, `tests/switch.rs`, and other files move to bound `src` directories or go away.
- `README.md`: the API, the overlay rows, the bind section, `state.json`, and the restart text.
- Hosts: each `src` export keeps `crossmnt`. vm-herd calls `bind` when it creates a host
  directory and `unbind` before it deletes one.
