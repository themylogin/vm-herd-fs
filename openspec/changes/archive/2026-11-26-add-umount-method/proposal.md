# Proposal

## Why

The caller can mount and switch a view but cannot take it back. After a reboot and a daemon start, each `dst` is in a known state: a restored view with only overlay directories when `state.json` holds it, otherwise a plain directory. The caller needs one call that puts a `dst` into that state without a reboot.

## What Changes

- Add the JSON-RPC method `umount` with the param `dst`. A successful call returns `null`.
- When `state.json` holds an item for `dst`, `umount` replaces the view in place with the restored view: no `src`, only the overlay directories, the stored `squash`. The mount stays, and project and `.idea` paths never disappear.
- When `state.json` holds no item for `dst`, `umount` unmounts the view, and `dst` is a plain directory again.
- `umount` does not change `state.json`, because a reboot does not change it either.
- `umount` for a `dst` that the daemon does not serve fails with a JSON-RPC error.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: adds the `umount` method.

## Impact

- `src/api.rs`: the `umount` method and its params type.
- `src/mounts.rs`: `Registry::umount`, and a restored-config builder shared with the restore at start.
- `README.md`: the new method.
