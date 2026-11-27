# Spec Delta

## ADDED Requirements

### Requirement: Fallback entries

For each project `<p>` and each entry `<e>` of `fallback.src`, `dst/<p>/<e>` SHALL show `fallback.src/<e>` when `src/<p>/<e>` does not exist and `<e>` is not in `overlay.directories`. This SHALL also hold in a project that exists only in `overlay.src`. A new or removed entry of `fallback.src`, and a `src/<p>/<e>` that appears or goes away, SHALL be visible within 1 second. With `fallback` `null`, no fallback entries SHALL show.

#### Scenario: Project without .git
- **GIVEN** `/w/fallback` holds the directory `.git` with the file `HEAD`
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** `/d/proj1` lists `.git`, `.idea`, and `main.py`
- **AND** `/d/proj1/.git/HEAD` shows `/w/fallback/.git/HEAD`

#### Scenario: Git of src wins
- **GIVEN** `/w/fallback` holds the directory `.git` with the file `HEAD`, and `/w/h1/proj1/.git` holds the file `config` only
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** `/d/proj1/.git` lists `config` only

#### Scenario: Overlay name wins
- **GIVEN** `/w/fallback` holds the directory `.idea` with the file `other.xml`
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** `/d/proj1/.idea` shows `/w/overlay/proj1/.idea`
- **AND** `/d/proj1/.idea` does not contain `other.xml`

#### Scenario: Project missing in src
- **GIVEN** `/w/fallback` holds the directory `.git`
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** `/d/proj2` lists `.git` and `.idea` only

#### Scenario: New entry in the fallback store
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`
- **WHEN** a process creates the file `/w/fallback/.envrc`
- **THEN** within 1 second `/d/proj1` and `/d/proj3` list `.envrc`

#### Scenario: Git appears in src
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback` holds `.git`
- **WHEN** an NFS client creates `/w/h1/proj3/.git` with the file `config`
- **THEN** within 1 second `/d/proj3/.git` lists `config` only

#### Scenario: No fallback
- **GIVEN** `/w/fallback` holds the directory `.git`
- **WHEN** a client calls mount h1 with `fallback` `null`
- **THEN** `/d/proj1` lists `.idea` and `main.py` only

### Requirement: Read-only fallback entries

A fallback entry and every path inside it SHALL be read-only through the view. Writes, creates, removes, renames, hard links, and changes of mode, owner, times, size, or xattrs SHALL fail with `EROFS`, with and without `squash`. An open for writing SHALL fail with `EROFS`. Reads, listings, `readlink`, and `stat` SHALL work. The view SHALL never change `fallback.src`.

#### Scenario: Write into a fallback file
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git/HEAD` exists
- **WHEN** UID 1000 opens `/d/proj1/.git/HEAD` for writing
- **THEN** the call fails with `EROFS`
- **AND** `/w/fallback/.git/HEAD` keeps its content

#### Scenario: Create inside a fallback directory
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git` exists
- **WHEN** a process creates `/d/proj1/.git/index.lock`
- **THEN** the call fails with `EROFS`
- **AND** `/w/fallback/.git/index.lock` does not exist

#### Scenario: Remove a fallback entry
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git/HEAD` exists
- **WHEN** a process runs `rm -rf /d/proj1/.git`
- **THEN** the command fails with `EROFS`
- **AND** `/w/fallback/.git/HEAD` exists

#### Scenario: Chmod with squash
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git/HEAD` exists
- **WHEN** root runs `chmod 600 /d/proj1/.git/HEAD`
- **THEN** the call fails with `EROFS`

#### Scenario: Read a fallback file
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git/HEAD` holds `ref: refs/heads/main`
- **WHEN** UID 1000 reads `/d/proj1/.git/HEAD`
- **THEN** the read returns `ref: refs/heads/main`

### Requirement: Fallback inode numbers

Each project SHALL show its own inode numbers for its fallback entries and for every path inside them. An in-place config change, including `umount` back to the restored view, SHALL keep these inode numbers when `fallback.src` stays the same.

#### Scenario: Two projects
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and `/w/fallback/.git/HEAD` exists
- **WHEN** a process calls `stat` on `/d/proj1/.git`, `/d/proj3/.git`, `/d/proj1/.git/HEAD`, and `/d/proj3/.git/HEAD`
- **THEN** the two `.git` inode numbers differ
- **AND** the two `HEAD` inode numbers differ

#### Scenario: Change of host
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and a process called `stat` on `/d/proj1/.git`
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay, squash, and fallback
- **THEN** `/d/proj1/.git` has the same inode number as before the call

#### Scenario: Back to the restored view
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and a process called `stat` on `/d/proj1/.git`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/d/proj1/.git` has the same inode number as before the call

## MODIFIED Requirements

### Requirement: JSON-RPC API

The daemon SHALL listen on the `host:port` given at start. It SHALL accept JSON-RPC 2.0 requests at `POST /api`. The method `mount` SHALL take the params `type`, `src`, `overlay`, `fallback`, `dst`, and `squash`. `overlay`, `fallback`, and `squash` MAY be `null`. An absent `overlay` or `fallback` SHALL mean `null`. `overlay`, when not `null`, SHALL hold `src` and `directories`. `fallback`, when not `null`, SHALL hold `src`. A successful `mount` SHALL return the result `null`.

#### Scenario: Successful mount
- **WHEN** a client sends `POST /api` with the method `mount` and valid params
- **THEN** the response has the result `null`
- **AND** `dst` shows the view

#### Scenario: Overlay omitted
- **WHEN** a client calls `mount` with `type` `cattle`, `src` `/w/h1`, `dst` `/d`, and `squash` `null`, and no `overlay` key
- **THEN** the response has the result `null`
- **AND** `/d` lists `proj1` and `proj3` only

#### Scenario: Fallback omitted
- **WHEN** a client calls mount h1 with no `fallback` key
- **THEN** the response has the result `null`
- **AND** `/d/proj1` lists `.idea` and `main.py` only

#### Scenario: Unknown method
- **WHEN** a client sends `POST /api` with the method `unmount`
- **THEN** the response has the JSON-RPC error code `-32601`

### Requirement: Param checks

Before any change, `mount` and `bind` SHALL return the JSON-RPC error `-32602` when a param is wrong. `overlay`, when present and not `null`, must be an object with exactly the members `src` and `directories`. `fallback`, when present and not `null`, must be an object with exactly the member `src`. The `mount` params `src`, `overlay.src`, and `fallback.src`, and the `bind` params `src` and `dst`, must be absolute paths of existing directories. `dst` must be an absolute path. When `dst` exists, it must be a directory. When `dst` does not exist, its parent must be an existing directory. Each `overlay.directories` item must be a plain name, without `/`, and not `.` or `..`. `dst` must not be inside `src`, `overlay.src`, or `fallback.src`, and none of them may be inside `dst`. `dst` must not be inside another served `dst`, and must not contain one. The checks use resolved paths.

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

#### Scenario: Missing fallback src
- **GIVEN** `/w/gone` does not exist
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/gone"}`
- **THEN** the response has the error code `-32602`
- **AND** the error message names `fallback.src`
- **AND** `/d` is not a mount point

#### Scenario: Fallback with an unknown member
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback", "directories": [".git"]}`
- **THEN** the response has the error code `-32602`

#### Scenario: dst inside the fallback store
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}` and `dst` `/w/fallback/d`
- **THEN** the response has the error code `-32602`

#### Scenario: Relative shared src
- **WHEN** a client calls `bind` with `src` `w/shared` and `dst` `/w/h1`
- **THEN** the response has the error code `-32602`

### Requirement: Mount type

The `mount` param `type` SHALL be `cattle` or `pet`. A missing `type` or another value SHALL fail with the JSON-RPC error `-32602` before any change. With `type` `pet`, `overlay` and `fallback` MUST be absent or `null`; otherwise `mount` SHALL fail with `-32602`.

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

#### Scenario: Pet with a fallback
- **WHEN** a client calls mount pet h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** the response has the error code `-32602`
- **AND** the error message names `fallback`
- **AND** `/d` is not a mount point

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1`, a `mounts` array, and a `binds` array. Each `mounts` item SHALL hold `type` and `dst`, with at most one item per `dst`. A `cattle` item SHALL also hold `overlay`, `fallback`, and `squash`. A `cattle` item without the `fallback` key SHALL load with `fallback` `null`. A `pet` item SHALL also hold `src` and `squash`. Each `binds` item SHALL hold `src` and `dst`, with at most one item per `dst`. The values SHALL have the form of the method params. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "fallback": null, "squash": {"uid": 1000, "gid": 1000}}], "binds": []}`

#### Scenario: Mount with a fallback
- **WHEN** a client calls mount h1 with `fallback` `{"src": "/w/fallback"}`
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "fallback": {"src": "/w/fallback"}, "squash": {"uid": 1000, "gid": 1000}}], "binds": []}`

#### Scenario: Cattle item without fallback key
- **GIVEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "squash": null}], "binds": []}`
- **WHEN** the daemon starts
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3` only
- **AND** `/d/proj1` contains only `.idea`

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

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. For a `cattle` item, the restored view SHALL list the directories of `overlay.src` as projects. Each project SHALL follow the rules for a project that exists only in `overlay.src`, and SHALL show the fallback entries of the stored `fallback.src`. Creates at the root of the view SHALL fail with `EROFS`. For a `pet` item, the restored view SHALL be the pet view of the stored `src`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

#### Scenario: Restart with an overlay
- **GIVEN** h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3` only
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea/workspace.xml` shows `/w/overlay/proj1/.idea/workspace.xml`

#### Scenario: Restart with a fallback
- **GIVEN** `/w/fallback` holds the directory `.git` with the file `HEAD`, h1 was mounted at `/d` with `fallback` `{"src": "/w/fallback"}`, and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d/proj1` lists `.git` and `.idea` only
- **AND** `/d/proj1/.git/HEAD` shows `/w/fallback/.git/HEAD`

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

### Requirement: Mount points at start

Only one daemon runs on a host. Before it restores a view, the daemon SHALL detach a `vm-herd-fs` mount at the item's `dst`, because an earlier run left it. When `dst` does not exist, when `dst` or a path below it holds another mount, when a pet item's `src` is not a directory, or when a cattle item's `fallback.src` is not a directory, the daemon SHALL skip the item, print the reason, and restore the other items. Skipped items SHALL stay in `state.json`.

#### Scenario: Restart after a crash
- **GIVEN** h1 was mounted at `/d` and the daemon got `SIGKILL`
- **WHEN** the daemon starts again
- **THEN** `/d` holds the restored view

#### Scenario: Foreign mount at a stored dst
- **GIVEN** `state.json` holds items for `/d` and `/d2`, and a tmpfs is mounted at `/d2`
- **WHEN** the daemon starts
- **THEN** `/d` holds the restored view
- **AND** the tmpfs at `/d2` stays
- **AND** `state.json` still holds the item for `/d2`

#### Scenario: Pet src missing at start
- **GIVEN** `state.json` holds a pet item for `/d` with `src` `/w/gone`, and `/w/gone` does not exist
- **WHEN** the daemon starts
- **THEN** `/d` is not a mount point
- **AND** the daemon prints a reason that names `/d`
- **AND** `state.json` still holds the item for `/d`

#### Scenario: Fallback src missing at start
- **GIVEN** `state.json` holds a cattle item for `/d` with `fallback` `{"src": "/w/gone"}`, and `/w/gone` does not exist
- **WHEN** the daemon starts
- **THEN** `/d` is not a mount point
- **AND** the daemon prints a reason that names `/d`
- **AND** `state.json` still holds the item for `/d`

### Requirement: Mounts items

Each `mounts` item SHALL hold `type`, `src`, `overlay`, `fallback`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored cattle view. A pet item SHALL hold its `src`, `overlay` `null`, and `fallback` `null`. Bind mounts SHALL not show in `mounts`.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "cattle", "src": "/w/h1", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "fallback": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Mounted host with a fallback
- **GIVEN** h1 is mounted at `/d` with `fallback` `{"src": "/w/fallback"}`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "cattle", "src": "/w/h1", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "fallback": {"src": "/w/fallback"}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Pet after a restart
- **GIVEN** pet h1 was mounted at `/d` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": null, "fallback": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Shared pet after a restart
- **GIVEN** a client called bind h1, pet h1 was mounted at `/d`, and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": null, "fallback": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Change of host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` and then calls `mounts`
- **THEN** the result has one item, and its `src` is `/w/h2`

#### Scenario: Two views
- **GIVEN** h1 is mounted at `/d` and `/w/h2` is mounted at `/d2`
- **WHEN** a client calls `mounts`
- **THEN** the result has the item for `/d` first and the item for `/d2` second

#### Scenario: Restored view after a restart
- **GIVEN** h1 was mounted at `/d` with `fallback` `{"src": "/w/fallback"}` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the item for `/d` has `src` `null` and the stored `overlay`, `fallback`, and `squash`

#### Scenario: Restored view after umount
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` for `/d` and then calls `mounts`
- **THEN** the item for `/d` has `src` `null`

#### Scenario: Unmounted view
- **GIVEN** `/w/h1` is mounted at `/d` with `overlay` `null`
- **WHEN** a client calls `umount` for `/d` and then calls `mounts`
- **THEN** the result has no item for `/d`
