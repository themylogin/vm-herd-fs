# Proposal

## Why

The caller cannot ask the daemon what it serves. After a daemon restart, or when the caller itself restarts, it has to guess which `dst` paths show a host and which show only the restored overlay directories. A read-only method that lists the current views removes the guess.

## What Changes

- Add the JSON-RPC method `mounts`. It takes no params.
- The result is an array with one item per served `dst`, sorted by `dst`. Each item has the form of the `mount` params: `src`, `overlay`, `dst`, and `squash`.
- `src` is `null` for a restored view: after a daemon start, or after `umount` of a `dst` with a stored overlay.
- A `dst` that `umount` unmounted, or that the restore skipped, is not in the result.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: adds the `mounts` method.

## Impact

- `src/mounts.rs`: each served `dst` records the config it shows, and `Registry::mounts` returns the list.
- `src/api.rs`: the `mounts` method and its result type.
- `README.md`: the new method.
