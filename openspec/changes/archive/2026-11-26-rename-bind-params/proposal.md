# Proposal

## Why

`bind` names the host directory `src` and the store `shared`, but `mount` uses `src` for what
the view shows and `dst` for where it shows it. A bind mount also goes from a source to a
target. The names must follow the same rule.

## What Changes

- **BREAKING** `bind` takes `src`, the store, and `dst`, the host directory.
- **BREAKING** `unbind` takes `dst`.
- **BREAKING** Each `binds` item in `state.json` and in the `binds` result is
  `{"src", "dst"}`, sorted by `dst`. An existing `state.json` with `binds` items has the old
  shape, and the user removes it once. `version` stays `1`.
- Error messages name the new params.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: the requirements Param checks, State file format, Bind method, Bind again,
  Bind mount points, Unbind method, Binds in the state file, Binds and views are independent,
  Binds at start, and Binds method use the new names.

## Impact

- `src/api.rs`, `src/state.rs`, `src/mounts.rs`: the param and field names, the key of the
  stored pairs, and the error messages.
- `tests/`: every `bind`, `unbind`, and `binds` call and every `binds` literal.
- `README.md`: the `bind`, `unbind`, `binds`, and `state.json` sections.
