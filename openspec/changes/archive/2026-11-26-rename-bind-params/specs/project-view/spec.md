# Spec Delta

## MODIFIED Requirements

### Requirement: Param checks

Before any change, `mount` and `bind` SHALL return the JSON-RPC error `-32602` when a param is wrong. `overlay`, when present and not `null`, must be an object with exactly the members `src` and `directories`. The `mount` params `src` and `overlay.src`, and the `bind` params `src` and `dst`, must be absolute paths of existing directories. `dst` must be an absolute path. When `dst` exists, it must be a directory. When `dst` does not exist, its parent must be an existing directory. Each `overlay.directories` item must be a plain name, without `/`, and not `.` or `..`. `dst` must not be inside `src` or `overlay.src`, and neither may be inside `dst`. `dst` must not be inside another served `dst`, and must not contain one. The checks use resolved paths.

#### Scenario: Relative path
- **WHEN** a client calls `mount` with `src` `w/h1`
- **THEN** the response has the error code `-32602`
- **AND** `/d` does not change

#### Scenario: dst inside src
- **WHEN** a client calls `mount` with `src` `/w/h1` and `dst` `/w/h1/view`
- **THEN** the response has the error code `-32602`

#### Scenario: dst with a missing parent
- **GIVEN** `/missing` does not exist
- **WHEN** a client calls `mount` with `src` `/w/h1` and `dst` `/missing/d`
- **THEN** the response has the error code `-32602`
- **AND** `/missing` does not exist

#### Scenario: dst is a file
- **GIVEN** `/f` is a regular file
- **WHEN** a client calls `mount` with `src` `/w/h1` and `dst` `/f`
- **THEN** the response has the error code `-32602`

#### Scenario: Name with a slash
- **WHEN** a client calls `mount` with `overlay.directories` `["a/b"]`
- **THEN** the response has the error code `-32602`

#### Scenario: Overlay without members
- **WHEN** a client calls `mount` with `overlay` `{"individual": {"src": "/w/overlay", "directories": [".idea"]}}`
- **THEN** the response has the error code `-32602`

#### Scenario: Relative shared src
- **WHEN** a client calls `bind` with `src` `w/shared` and `dst` `/w/h1`
- **THEN** the response has the error code `-32602`

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1`, a `mounts` array, and a `binds` array. Each `mounts` item SHALL hold `type` and `dst`, with at most one item per `dst`. A `cattle` item SHALL also hold `overlay` and `squash`. A `pet` item SHALL also hold `src` and `squash`. Each `binds` item SHALL hold `src` and `dst`, with at most one item per `dst`. The values SHALL have the form of the method params. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "squash": {"uid": 1000, "gid": 1000}}], "binds": []}`

#### Scenario: Pet mount
- **WHEN** a client calls mount pet h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "squash": {"uid": 1000, "gid": 1000}}], "binds": []}`

#### Scenario: Pet mount with a shared overlay
- **WHEN** a client calls mount pet h1 with `overlay` `{"shared": {"src": "/w/shared"}}`
- **THEN** the response has the error code `-32602`
- **AND** `state.json` does not change

#### Scenario: Pet item without overlay key
- **GIVEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "squash": null}], "binds": []}`
- **WHEN** the daemon starts
- **THEN** `/d` lists `proj1` and `proj3` only

### Requirement: Bind method

The daemon SHALL accept the JSON-RPC method `bind` with the params `src` and `dst`. A successful call SHALL return `null`. After it, each file and directory `<e>` that `src` holds at the time of the call SHALL be bind-mounted at `dst/<e>`. A process that reads or writes `dst/<e>` or a path below it SHALL act on `src/<e>`. A bind mount SHALL hide the `dst` entry of the same name and leave it unchanged on disk.

#### Scenario: Shared file in src
- **WHEN** a client calls bind h1
- **THEN** the response has the result `null`
- **AND** `/w/h1/notes.md` is a mount point with the content of `/w/shared/notes.md`

#### Scenario: Create inside a shared directory in src
- **GIVEN** a client called bind h1
- **WHEN** a process creates `/w/h1/tools/new.sh`
- **THEN** `/w/shared/tools/new.sh` exists

#### Scenario: Remove inside a shared directory in the store
- **GIVEN** a client called bind h1
- **WHEN** a process removes `/w/shared/tools/run.sh`
- **THEN** `/w/h1/tools/run.sh` does not exist

#### Scenario: Shared entry hides a src entry
- **GIVEN** `/w/h1/notes.md` holds `host notes`
- **WHEN** a client calls bind h1
- **THEN** `/w/h1/notes.md` has the content of `/w/shared/notes.md`
- **AND** after `unbind` of `/w/h1`, `/w/h1/notes.md` holds `host notes`

#### Scenario: Two hosts share one store
- **WHEN** a client calls bind h1 and then `bind` with `dst` `/w/shared` and `dst` `/w/h2`
- **THEN** `/w/h1/notes.md` and `/w/h2/notes.md` are mount points

### Requirement: Bind again

A `bind` for a `dst` that has bind mounts SHALL make them match the entries that `src` holds now. It SHALL add a bind mount for each new entry, remove the bind mounts of entries that are gone or that come from another store, and keep the others. It SHALL never put a second bind mount on a path that has one.

#### Scenario: Same store keeps one bind mount
- **GIVEN** a client called bind h1
- **WHEN** a client calls bind h1 again
- **THEN** `/proc/self/mountinfo` has one mount at `/w/h1/notes.md`

#### Scenario: Store entry added after the bind
- **GIVEN** a client called bind h1 and a process created `/w/shared/new.md`
- **WHEN** a client calls bind h1 again
- **THEN** `/w/h1/new.md` is a mount point

#### Scenario: Store entry removed after the bind
- **GIVEN** a client called bind h1 and a process removed `/w/shared/notes.md`
- **WHEN** a client calls bind h1 again
- **THEN** `/w/h1/notes.md` does not exist

#### Scenario: Another store
- **GIVEN** a client called bind h1 and `/w/other` holds the file `notes.md` only
- **WHEN** a client calls `bind` with `dst` `/w/other` and `dst` `/w/h1`
- **THEN** `/w/h1/notes.md` has the content of `/w/other/notes.md`
- **AND** `/w/h1/tools` does not exist
- **AND** `/proc/self/mountinfo` has one mount at `/w/h1/notes.md`

### Requirement: Bind mount points

When `dst/<e>` does not exist, `bind` SHALL create it as an empty file with mode `0644` or an empty directory with mode `0755`, of the same kind as `src/<e>`, with owner 0:0. `bind` SHALL fail with `-32000` and change nothing when `dst/<e>` is a symlink or of the other kind, or when `<e>` is not a file or a directory. The message SHALL name the path.

#### Scenario: Missing mount point
- **GIVEN** `/w/h1/tools` does not exist
- **WHEN** a client calls bind h1
- **THEN** `/w/h1/tools` is a mount point that lists `run.sh`

#### Scenario: Symlink at the mount point
- **GIVEN** `/w/h1/tools` is a symlink to `/w/h2`
- **WHEN** a client calls bind h1
- **THEN** the response has the error code `-32000` and the message names `/w/h1/tools`
- **AND** `/w/h1/notes.md` and `/w/h2` are not mount points

#### Scenario: File where the store has a directory
- **GIVEN** `/w/h1/tools` is a file
- **WHEN** a client calls bind h1
- **THEN** the response has the error code `-32000` and the message names `/w/h1/tools`
- **AND** `/w/h1/notes.md` is not a mount point

#### Scenario: Store entry of another kind
- **GIVEN** `/w/shared/link` is a symlink
- **WHEN** a client calls bind h1
- **THEN** the response has the error code `-32000` and the message names `/w/shared/link`

#### Scenario: Failed bind again keeps the bind mounts
- **GIVEN** a client called bind h1, and then `/w/shared/link` became a symlink
- **WHEN** a client calls bind h1 again
- **THEN** the response has the error code `-32000`
- **AND** `/w/h1/notes.md` is a mount point

### Requirement: Unbind method

The daemon SHALL accept the JSON-RPC method `unbind` with the param `dst`, an absolute path. It SHALL remove each bind mount of `dst`, then remove `dst/<e>` when that is an empty directory or a file of size 0, and return `null`. Open files and NFS clients SHALL not block the call. When `dst` has no bind mounts and no item in `state.json`, `unbind` SHALL fail with `-32000` and change nothing.

#### Scenario: Unbind
- **GIVEN** a client called bind h1
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `/w/h1/notes.md` and `/w/h1/tools` do not exist

#### Scenario: Entry with content stays
- **GIVEN** `/w/h1/notes.md` holds `host notes` and a client called bind h1
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** `/w/h1/notes.md` holds `host notes`

#### Scenario: Open file during unbind
- **GIVEN** a client called bind h1 and a process holds `/w/h1/tools/run.sh` open
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `/w/h1/tools` is not a mount point

#### Scenario: Src without bind mounts
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** the response has the error code `-32000`

#### Scenario: Stored item that failed at start
- **GIVEN** a client called bind h1, the daemon stopped, the bind mounts at `/w/h1` were removed, `/w/h1` was deleted, and the daemon started again
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `state.json` holds `"binds": []`

#### Scenario: Relative src
- **WHEN** a client calls `unbind` with `dst` `w/h1`
- **THEN** the response has the error code `-32602`

### Requirement: Binds in the state file

After each successful `bind`, `state.json` SHALL hold the item `{"src", "dst"}` of that call for its `dst`, with resolved paths. After each successful `unbind`, it SHALL hold no item for that `dst`. A failed `bind` or `unbind` SHALL not change the file. `mount` and `umount` SHALL not change `binds`.

#### Scenario: Bind is stored
- **WHEN** a client calls bind h1
- **THEN** `state.json` holds `"binds": [{"src": "/w/shared", "dst": "/w/h1"}]`

#### Scenario: Unbind is stored
- **GIVEN** a client called bind h1
- **WHEN** a client calls `unbind` with `dst` `/w/h1`
- **THEN** `state.json` holds `"binds": []`

### Requirement: Binds at start

Before it restores a view, the daemon SHALL bind each `binds` item of `state.json` with the rules of the requirement Bind again, so a bind mount that an earlier run left stays and is never doubled. When an item fails, the daemon SHALL print the reason, keep the item in `state.json`, and go on with the other items.

#### Scenario: Restart after a crash
- **GIVEN** a client called bind h1 and the daemon got `SIGKILL`
- **WHEN** the daemon starts again
- **THEN** `/proc/self/mountinfo` has one mount at `/w/h1/notes.md`

#### Scenario: Restart after the bind mounts went away
- **GIVEN** a client called bind h1, the daemon stopped, and the bind mounts at `/w/h1` were removed
- **WHEN** the daemon starts again
- **THEN** `/w/h1/notes.md` is a mount point with the content of `/w/shared/notes.md`

#### Scenario: Broken mount point at start
- **GIVEN** `state.json` holds `"binds": [{"src": "/w/shared", "dst": "/w/h1"}]` and `/w/h1/tools` is a file
- **WHEN** the daemon starts
- **THEN** the daemon prints a reason that names `/w/h1/tools`
- **AND** `state.json` still holds the item

### Requirement: Binds method

The daemon SHALL accept the JSON-RPC method `binds` without params. The params MAY be absent, `null`, `{}`, or `[]`. Other params SHALL fail with the JSON-RPC error `-32602`. The result SHALL be an array with one item `{"src", "dst"}` per `binds` item of `state.json`, sorted by `dst`, with the paths of that item. An item whose bind failed at start SHALL be in the result.

#### Scenario: No binds
- **GIVEN** no client called `bind`
- **WHEN** a client calls `binds`
- **THEN** the result is `[]`

#### Scenario: Bound host
- **GIVEN** a client called bind h1
- **WHEN** a client calls `binds`
- **THEN** the result is `[{"src": "/w/shared", "dst": "/w/h1"}]`

#### Scenario: Two hosts
- **GIVEN** a client called `bind` with `dst` `/w/shared` and `dst` `/w/h2`, and then bind h1
- **WHEN** a client calls `binds`
- **THEN** the result has the item for `/w/h1` first and the item for `/w/h2` second

#### Scenario: After unbind
- **GIVEN** a client called bind h1 and then `unbind` with `dst` `/w/h1`
- **WHEN** a client calls `binds`
- **THEN** the result is `[]`

#### Scenario: Item that failed at start
- **GIVEN** `state.json` holds `"binds": [{"src": "/w/shared", "dst": "/w/h1"}]` and `/w/h1/tools` is a file
- **WHEN** the daemon starts and a client calls `binds`
- **THEN** the result is `[{"src": "/w/shared", "dst": "/w/h1"}]`

#### Scenario: Unexpected params
- **WHEN** a client calls `binds` with the params `{"dst": "/w/h1"}`
- **THEN** the response has the error code `-32602`

### Requirement: Binds and views are independent

`mount` and `umount` SHALL never add or remove a bind mount. `bind` and `unbind` SHALL never change a view config. A view SHALL show a bind mount in its `src` like any other entry of `src`.

#### Scenario: Change of host keeps the bind mounts
- **GIVEN** a client called bind h1 and `bind` with `src` `/w/shared` and `dst` `/w/h2`, and h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** `/w/h1/notes.md` and `/w/h2/notes.md` are mount points

#### Scenario: Shared entry keeps its inode across a change of host
- **GIVEN** both `/w/h1` and `/w/h2` are bound to `/w/shared`, h1 is mounted at `/d`, and a process calls `stat` on `/d/tools` in a loop
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** every `stat` call succeeds and returns the same inode number

#### Scenario: Umount keeps the bind mounts
- **GIVEN** a client called bind h1 and h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/w/h1/notes.md` is a mount point

#### Scenario: Bind under a served view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls bind h1
- **THEN** within 1 second `/d` lists `notes.md`, `proj1`, `proj2`, `proj3`, and `tools`
