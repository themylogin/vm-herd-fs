# Proposal

## Why

PyCharm loses the Git root of a project when `.git` disappears from the view. This happens in
the restored view after `umount` or a daemon start, and for a host project without `.git`. An
uncommitted experiment shows the hardcoded `/home/themylogin/test/.git` in each project, and
PyCharm keeps the root with it. The path must become a param, and the entries must not change.

## What Changes

- `mount` with `type` `cattle` accepts the param `fallback`: `null`, or `{"src": <dir>}`.
  Absent means `null`. A pet rejects a `fallback` that is not `null` with `-32602`.
- Each entry `<e>` of `fallback.src` shows as `dst/<p>/<e>` in each project `<p>`, when
  `src/<p>/<e>` does not exist and `<e>` is not in `overlay.directories`. A real entry of `src`
  always wins. This includes `.git`, and any other entry put in the store later.
- Fallback entries are read-only. Writes, creates, removes, renames, and metadata changes in
  them fail with `EROFS`. A create of `dst/<p>/<e>` in a project with `src` still creates
  `src/<p>/<e>`, which then hides the fallback entry.
- Each project gets its own inode numbers for the fallback entries and everything inside them.
- The cattle item in `state.json` holds `fallback`. An item without the key loads as `null`, so
  an existing `state.json` keeps working. `version` stays `1`.
- The restored view after a start or `umount` shows the stored fallback entries in each
  project. A missing `fallback.src` at start skips the item, like a missing pet `src`.
- A `mounts` item holds `fallback`.
- The hardcoded `/home/themylogin/test/.git` and its README paragraph go away.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-view`: the requirements JSON-RPC API, Param checks, Mount type, State file format,
  Restore at start, Mount points at start, and Mounts items change to carry `fallback`. New
  requirements describe fallback entries, their read-only access, and their inode numbers.

## Impact

- `src/api.rs`: `MountParams` and `MountsItem` gain `fallback`.
- `src/state.rs`: `StoredMount::Cattle` gains `fallback`.
- `src/mounts.rs`: param checks, `Config` setup at mount and restore, `Shown`, and the stored
  cattle item. The hardcoded path goes away.
- `src/view.rs`: `Config.fallback_git` becomes the store descriptor. Lookups and listings show
  all store entries. Each mutating call refuses nodes of a fallback realm.
- `src/nodes.rs`: no change beyond the realm that the experiment added.
- `tests/`: new tests for the param, the view, read-only access, state, restore, and `mounts`.
  Existing `mounts` and `state.json` literals gain `"fallback": null`.
- `README.md`: the param table, the cattle example, the `state.json` example, and the `mounts`
  example.
