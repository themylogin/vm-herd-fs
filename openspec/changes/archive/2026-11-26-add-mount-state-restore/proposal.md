# Proposal

## Why

When the daemon stops or crashes, each `dst` becomes a dead mount, and after a restart it is an empty directory. The IDE then loses every project and its `.idea`, which is the failure the view exists to prevent. The daemon must remember the overlays it serves and bring the `.idea` directories back at start, before the caller sends `mount` again.

## What Changes

- Add the required start flag `--data-dir`. The daemon creates the directory when it is missing.
- The daemon keeps `<data-dir>/state.json`, a JSON file with `version` and `mounts`. Each successful `mount` with `overlay` stores `dst`, `overlay`, and `squash` for that `dst`. A `mount` without `overlay` removes the `dst` from the file.
- At start, the daemon mounts a restored view for each stored `dst`. A restored view has no `src`: it lists the projects of `overlay.src`, and each project holds only its existing overlay directories.
- At start, the daemon detaches a `vm-herd-fs` mount at a stored `dst`, which an earlier run left, before it mounts the restored view. Only one daemon runs on a host, so the daemon does not check whether that mount is alive. A stored `dst` that is missing or holds a foreign mount is skipped with a log line.
- A later `mount` for a restored `dst` replaces the config in place, as for any served `dst`.
- **BREAKING**: the daemon does not start without `--data-dir`, and it does not start when `state.json` has an unknown `version`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: adds the state file, the `--data-dir` flag, and the restore of overlay directories at start.

## Impact

- `src/main.rs`: the new flag and the restore before the API starts.
- `src/mounts.rs`: the state file reads and writes, the restore, and the detach of earlier `vm-herd-fs` mounts.
- `src/view.rs`: a view config without `src`.
- `README.md`: the new flag, the state file, and the new behavior after a stop.
- The README build and start commands change, because `--data-dir` is required.
