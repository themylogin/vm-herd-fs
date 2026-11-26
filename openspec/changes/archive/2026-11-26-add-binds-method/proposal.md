# Proposal

## Why

A client cannot ask the daemon which host directories it binds. Today the user reads the
`binds` array of `state.json` or the kernel mount table, and neither one comes through the API.

## What Changes

- New JSON-RPC method `binds` without params, like `mounts`. Its result has one item
  `{"src", "shared"}` per stored bind, sorted by `src`, with resolved paths.
- The result holds the same pairs as the `binds` array of `state.json`. A pair whose bind
  failed at start is in it too, because `unbind` accepts that pair.
- `-32601` names `binds` in the list of methods.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: a new requirement for the method `binds`.

## Impact

- `src/api.rs`: the method `binds`; the params check of `mounts` becomes one function that both
  methods call.
- `src/mounts.rs`: `Registry::binds` returns the stored pairs.
- `tests/binds.rs`: tests for the new method.
- `README.md`: a `binds` section and the `-32601` row.
