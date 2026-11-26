# Spec Delta

## ADDED Requirements

### Requirement: Bind method

The daemon SHALL accept the JSON-RPC method `bind` with the params `src` and `shared`. A successful call SHALL return `null`. After it, each file and directory `<e>` that `shared` holds at the time of the call SHALL be bind-mounted at `src/<e>`. A process that reads or writes `src/<e>` or a path below it SHALL act on `shared/<e>`. A bind mount SHALL hide the `src` entry of the same name and leave it unchanged on disk.

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
- **WHEN** a client calls bind h1 and then `bind` with `src` `/w/h2` and `shared` `/w/shared`
- **THEN** `/w/h1/notes.md` and `/w/h2/notes.md` are mount points

### Requirement: Bind again

A `bind` for a `src` that has bind mounts SHALL make them match the entries that `shared` holds now. It SHALL add a bind mount for each new entry, remove the bind mounts of entries that are gone or that come from another store, and keep the others. It SHALL never put a second bind mount on a path that has one.

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
- **WHEN** a client calls `bind` with `src` `/w/h1` and `shared` `/w/other`
- **THEN** `/w/h1/notes.md` has the content of `/w/other/notes.md`
- **AND** `/w/h1/tools` does not exist
- **AND** `/proc/self/mountinfo` has one mount at `/w/h1/notes.md`

### Requirement: Bind mount points

When `src/<e>` does not exist, `bind` SHALL create it as an empty file with mode `0644` or an empty directory with mode `0755`, of the same kind as `shared/<e>`, with owner 0:0. `bind` SHALL fail with `-32000` and change nothing when `src/<e>` is a symlink or of the other kind, or when `<e>` is not a file or a directory. The message SHALL name the path.

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

The daemon SHALL accept the JSON-RPC method `unbind` with the param `src`, an absolute path. It SHALL remove each bind mount of `src`, then remove `src/<e>` when that is an empty directory or a file of size 0, and return `null`. Open files and NFS clients SHALL not block the call. When `src` has no bind mounts and no item in `state.json`, `unbind` SHALL fail with `-32000` and change nothing.

#### Scenario: Unbind
- **GIVEN** a client called bind h1
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `/w/h1/notes.md` and `/w/h1/tools` do not exist

#### Scenario: Entry with content stays
- **GIVEN** `/w/h1/notes.md` holds `host notes` and a client called bind h1
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** `/w/h1/notes.md` holds `host notes`

#### Scenario: Open file during unbind
- **GIVEN** a client called bind h1 and a process holds `/w/h1/tools/run.sh` open
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `/w/h1/tools` is not a mount point

#### Scenario: Src without bind mounts
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** the response has the error code `-32000`

#### Scenario: Stored item that failed at start
- **GIVEN** a client called bind h1, the daemon stopped, the bind mounts at `/w/h1` were removed, `/w/h1` was deleted, and the daemon started again
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** the response has the result `null`
- **AND** `state.json` holds `"binds": []`

#### Scenario: Relative src
- **WHEN** a client calls `unbind` with `src` `w/h1`
- **THEN** the response has the error code `-32602`

### Requirement: Binds in the state file

After each successful `bind`, `state.json` SHALL hold the item `{"src", "shared"}` of that call for its `src`, with resolved paths. After each successful `unbind`, it SHALL hold no item for that `src`. A failed `bind` or `unbind` SHALL not change the file. `mount` and `umount` SHALL not change `binds`.

#### Scenario: Bind is stored
- **WHEN** a client calls bind h1
- **THEN** `state.json` holds `"binds": [{"src": "/w/h1", "shared": "/w/shared"}]`

#### Scenario: Unbind is stored
- **GIVEN** a client called bind h1
- **WHEN** a client calls `unbind` with `src` `/w/h1`
- **THEN** `state.json` holds `"binds": []`

### Requirement: Binds and views are independent

`mount` and `umount` SHALL never add or remove a bind mount. `bind` and `unbind` SHALL never change a view config. A view SHALL show a bind mount in its `src` like any other entry of `src`.

#### Scenario: Change of host keeps the bind mounts
- **GIVEN** a client called bind h1 and `bind` with `src` `/w/h2` and `shared` `/w/shared`, and h1 is mounted at `/d`
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
- **GIVEN** `state.json` holds `"binds": [{"src": "/w/h1", "shared": "/w/shared"}]` and `/w/h1/tools` is a file
- **WHEN** the daemon starts
- **THEN** the daemon prints a reason that names `/w/h1/tools`
- **AND** `state.json` still holds the item

## MODIFIED Requirements

### Requirement: JSON-RPC API

The daemon SHALL listen on the `host:port` given at start. It SHALL accept JSON-RPC 2.0 requests at `POST /api`. The method `mount` SHALL take the params `type`, `src`, `overlay`, `dst`, and `squash`. `overlay` and `squash` MAY be `null`. An absent `overlay` SHALL mean `null`. `overlay`, when not `null`, SHALL hold `src` and `directories`. A successful `mount` SHALL return the result `null`.

#### Scenario: Successful mount
- **WHEN** a client sends `POST /api` with the method `mount` and valid params
- **THEN** the response has the result `null`
- **AND** `dst` shows the view

#### Scenario: Overlay omitted
- **WHEN** a client calls `mount` with `type` `cattle`, `src` `/w/h1`, `dst` `/d`, and `squash` `null`, and no `overlay` key
- **THEN** the response has the result `null`
- **AND** `/d` lists `proj1` and `proj3` only

#### Scenario: Unknown method
- **WHEN** a client sends `POST /api` with the method `unmount`
- **THEN** the response has the JSON-RPC error code `-32601`

### Requirement: Param checks

Before any change, `mount` and `bind` SHALL return the JSON-RPC error `-32602` when a param is wrong. `overlay`, when present and not `null`, must be an object with exactly the members `src` and `directories`. `src`, `overlay.src`, and the `bind` param `shared` must be absolute paths of existing directories. `dst` must be an absolute path. When `dst` exists, it must be a directory. When `dst` does not exist, its parent must be an existing directory. Each `overlay.directories` item must be a plain name, without `/`, and not `.` or `..`. `dst` must not be inside `src` or `overlay.src`, and neither may be inside `dst`. `dst` must not be inside another served `dst`, and must not contain one. The checks use resolved paths.

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
- **WHEN** a client calls `bind` with `src` `/w/h1` and `shared` `w/shared`
- **THEN** the response has the error code `-32602`

### Requirement: Project list

The root of `dst` SHALL list the entries of `src` and the directories of `overlay.src` when it is set. A directory of `overlay.src` that `src` does not have SHALL show as a read-only project. An entry that a `bind` put at `src/<e>` SHALL show as `dst/<e>` like any other entry of `src`.

#### Scenario: Union of projects
- **WHEN** a client calls mount h1
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3`

#### Scenario: No overlay
- **WHEN** a client calls `mount` with `src` `/w/h1`, `overlay` `null`, and `dst` `/d`
- **THEN** `/d` lists `proj1` and `proj3` only

#### Scenario: Shared without individual
- **GIVEN** a client called bind h1
- **WHEN** a client calls `mount` with `src` `/w/h1`, `overlay` `null`, and `dst` `/d`
- **THEN** `/d` lists `notes.md`, `proj1`, `proj3`, and `tools` only
- **AND** `/d/tools` lists `run.sh` only

#### Scenario: New project in src
- **GIVEN** h1 is mounted at `/d`
- **WHEN** an NFS client creates `/w/h1/proj4` with the file `main.py`
- **THEN** the next listing of `/d` contains `proj4`
- **AND** `/d/proj4` shows `main.py` and `.idea`

### Requirement: Overlay directories

For each project `<p>` and each name `<n>` in `overlay.directories`, `dst/<p>/<n>` SHALL show `overlay.src/<p>/<n>`. Writes into it SHALL change `overlay.src/<p>/<n>`. When `overlay.src/<p>/<n>` does not exist, `dst/<p>/<n>` SHALL not exist. A create of `dst/<p>/<n>` SHALL create `overlay.src/<p>/<n>`, and `overlay.src/<p>` when it is missing. The daemon SHALL not create, read, or change `src/<p>/<n>`.

#### Scenario: Source has its own .idea
- **GIVEN** `/w/h1/proj1/.idea` holds other files
- **WHEN** a client calls mount h1 and a process lists `/d/proj1/.idea`
- **THEN** it sees the files of `/w/overlay/proj1/.idea` only

#### Scenario: Source has no .idea
- **GIVEN** `/w/h2/proj2` has no `.idea` directory
- **WHEN** a client mounts `/w/h2` at `/d` and a process lists `/d/proj2`
- **THEN** the list contains `.idea`
- **AND** `/w/h2/proj2/.idea` does not exist

#### Scenario: New project has no overlay directory
- **GIVEN** h1 is mounted at `/d`
- **WHEN** an NFS client creates `/w/h1/proj4`
- **AND** a process lists `/d/proj4`
- **THEN** the list is empty
- **AND** `/w/overlay/proj4` does not exist

#### Scenario: Source .idea without an overlay directory
- **GIVEN** h1 is mounted at `/d` and `/w/h1/proj4/.idea` exists
- **WHEN** a process lists `/d/proj4`
- **THEN** the list does not contain `.idea`

#### Scenario: IDE creates .idea
- **GIVEN** h1 is mounted at `/d` and `/w/h1/proj4` exists
- **WHEN** the IDE creates the directory `/d/proj4/.idea`
- **THEN** `/w/overlay/proj4/.idea` exists
- **AND** `/w/h1/proj4/.idea` does not exist

### Requirement: Read-only projects and fixed entries

A project that exists only in `overlay.src` SHALL contain only its overlay directories. Creates in it SHALL fail with `EROFS`, except creates of overlay directories. Removal or rename of such a project, or of an overlay directory entry `dst/<p>/<n>`, SHALL fail with `EPERM`.

#### Scenario: Project missing in src
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process lists `/d/proj2`
- **THEN** the list contains only `.idea`

#### Scenario: Write into a read-only project
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process creates `/d/proj2/new.txt`
- **THEN** the call fails with `EROFS`

#### Scenario: Remove an overlay directory
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process renames `/d/proj1/.idea` to `/d/proj1/idea-old`
- **THEN** the call fails with `EPERM`

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1`, a `mounts` array, and a `binds` array. Each `mounts` item SHALL hold `type` and `dst`, with at most one item per `dst`. A `cattle` item SHALL also hold `overlay` and `squash`. A `pet` item SHALL also hold `src` and `squash`. Each `binds` item SHALL hold `src` and `shared`, with at most one item per `src`. The values SHALL have the form of the method params. The file SHALL never be partly written.

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

### Requirement: Restore at start

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. For a `cattle` item, the restored view SHALL list the directories of `overlay.src` as projects. Each project SHALL follow the rules for a project that exists only in `overlay.src`. Creates at the root of the view SHALL fail with `EROFS`. For a `pet` item, the restored view SHALL be the pet view of the stored `src`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

#### Scenario: Restart with an overlay
- **GIVEN** h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3` only
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea/workspace.xml` shows `/w/overlay/proj1/.idea/workspace.xml`

#### Scenario: Restart with shared only
- **GIVEN** a client called bind h1, h1 was mounted at `/d`, and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3` only
- **AND** a write to `/w/h1/notes.md` changes `/w/shared/notes.md`

#### Scenario: Restart with a pet
- **GIVEN** pet h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1` and `proj3` only
- **AND** a write to `/d/proj1/main.py` changes `/w/h1/proj1/main.py`
- **AND** `/d/proj1/main.py` shows owner 1000:1000

#### Scenario: Restart with a shared pet
- **GIVEN** a client called bind h1, pet h1 was mounted at `/d`, and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `notes.md`, `proj1`, `proj3`, and `tools` only
- **AND** a write to `/d/notes.md` changes `/w/shared/notes.md`
- **AND** a write to `/d/proj1/main.py` changes `/w/h1/proj1/main.py`

#### Scenario: Create at the root of a restored view
- **GIVEN** the daemon restored the view at `/d`
- **WHEN** a process creates `/d/proj5`
- **THEN** the call fails with `EROFS`

#### Scenario: Mount after a restore
- **GIVEN** the daemon restored the view at `/d` and a process calls `stat` on `/d/proj1/.idea`
- **WHEN** a client calls mount h1
- **THEN** `/d/proj1` shows `/w/h1/proj1`
- **AND** `/d/proj1/.idea` has the same inode number as before the call

### Requirement: Umount with a stored overlay

When `state.json` holds a cattle item for `dst`, `umount` SHALL replace the view in place with the view that a daemon start restores for that item. Lookups of each project and overlay directory that exist in both views SHALL never fail during the call, and their inode numbers SHALL not change.

#### Scenario: Back to the restored view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3` only
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea` has the same inode number as before the call

#### Scenario: Mount after umount
- **GIVEN** h1 is mounted at `/d` and a client called `umount` for `/d`
- **WHEN** a client calls mount h1
- **THEN** `/d/proj1` shows `/w/h1/proj1`

### Requirement: Mounts items

Each `mounts` item SHALL hold `type`, `src`, `overlay`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored cattle view. A pet item SHALL hold its `src` and `overlay` `null`. Bind mounts SHALL not show in `mounts`.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "cattle", "src": "/w/h1", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Pet after a restart
- **GIVEN** pet h1 was mounted at `/d` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Shared pet after a restart
- **GIVEN** a client called bind h1, pet h1 was mounted at `/d`, and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Change of host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` and then calls `mounts`
- **THEN** the result has one item, and its `src` is `/w/h2`

#### Scenario: Two views
- **GIVEN** h1 is mounted at `/d` and `/w/h2` is mounted at `/d2`
- **WHEN** a client calls `mounts`
- **THEN** the result has the item for `/d` first and the item for `/d2` second

#### Scenario: Restored view after a restart
- **GIVEN** h1 was mounted at `/d` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the item for `/d` has `src` `null` and the stored `overlay` and `squash`

#### Scenario: Restored view after umount
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` for `/d` and then calls `mounts`
- **THEN** the item for `/d` has `src` `null`

#### Scenario: Unmounted view
- **GIVEN** `/w/h1` is mounted at `/d` with `overlay` `null`
- **WHEN** a client calls `umount` for `/d` and then calls `mounts`
- **THEN** the result has no item for `/d`

### Requirement: Mount type

The `mount` param `type` SHALL be `cattle` or `pet`. A missing `type` or another value SHALL fail with the JSON-RPC error `-32602` before any change. With `type` `pet`, `overlay` MUST be absent or `null`; otherwise `mount` SHALL fail with `-32602`.

#### Scenario: Missing type
- **WHEN** a client calls `mount` with `src` `/w/h1`, `dst` `/d`, and no `type` key
- **THEN** the response has the error code `-32602`
- **AND** `/d` is not a mount point

#### Scenario: Unknown type
- **WHEN** a client calls `mount` with `type` `sheep`
- **THEN** the response has the error code `-32602`

#### Scenario: Pet with an overlay
- **WHEN** a client calls `mount` with `type` `pet`, `src` `/w/h1`, `overlay` `{"src": "/w/overlay", "directories": [".idea"]}`, and `dst` `/d`
- **THEN** the response has the error code `-32602`
- **AND** the error message names `overlay`
- **AND** `/d` is not a mount point

#### Scenario: Pet with a shared overlay
- **WHEN** a client calls mount pet h1 with `overlay` `{"shared": {"src": "/w/shared"}}`
- **THEN** the response has the error code `-32602`
- **AND** `/d` is not a mount point

### Requirement: Pet view

With `type` `pet`, `dst` SHALL show `src` as a passthrough with `squash` applied, with no overlay directories. The requirements Passthrough of src, Source changes become visible, Ownership squash, and In-place config change SHALL apply to a pet view.

#### Scenario: Pet lists src only
- **WHEN** a client calls mount pet h1
- **THEN** `/d` lists `proj1` and `proj3` only
- **AND** `/d/proj1` lists `main.py` only

#### Scenario: Pet lists src and shared entries
- **GIVEN** a client called bind h1
- **WHEN** a client calls mount pet h1
- **THEN** `/d` lists `notes.md`, `proj1`, `proj3`, and `tools` only
- **AND** `/d/proj1` lists `main.py` only

#### Scenario: Write to a shared file in a pet view
- **GIVEN** a client called bind h1 and pet h1 is mounted at `/d`
- **WHEN** UID 1000 writes `/d/notes.md`
- **THEN** `/w/shared/notes.md` has the new content

#### Scenario: Shared directory hides a pet src directory
- **GIVEN** `/w/shared/proj3` is a directory with the file `a.txt`, and a client called bind h1
- **WHEN** a client calls mount pet h1
- **THEN** `/d/proj3` lists `a.txt` only

#### Scenario: Write through a pet view
- **GIVEN** pet h1 is mounted at `/d`
- **WHEN** UID 1000 writes `/d/proj1/main.py`
- **THEN** `/w/h1/proj1/main.py` has the new content
- **AND** `/d/proj1/main.py` shows owner 1000:1000

#### Scenario: Pet replaces a cattle view in place
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls mount pet h1
- **THEN** the call succeeds and `/d` stays a mount point
- **AND** `/d` lists `proj1` and `proj3` only

## REMOVED Requirements

### Requirement: Shared entries
**Reason**: Shared entries live in each host directory as bind mounts from `bind`, so the view shows them through the passthrough of `src`.
**Migration**: Call `bind` with `src` and the former `overlay.shared.src` for each host directory, and remove `overlay.shared` from `mount` calls.

### Requirement: Fixed shared entries
**Reason**: A top-level bound entry is a mount point, so the kernel already refuses its removal and a rename onto it, with `EBUSY`.
**Migration**: None. Clients that expect `EPERM` get `EBUSY`.
