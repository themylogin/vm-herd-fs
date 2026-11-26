# project-view Specification

## Purpose

Mounts a view that shows one host's project directories at a stable path. The view keeps IDE settings in a separate store, keeps projects of other hosts in place, and can map file ownership to one user.

The scenarios use these example paths. `/w/h1` holds `proj1` and `proj3`. `/w/h2` holds `proj1` and `proj2`. `/w/overlay` holds `proj1/.idea`, `proj2/.idea`, and `proj3/.idea`. `/w/shared` holds the file `notes.md` and the directory `tools` with `run.sh`. The view goes to `/d`. "Mount h1" means a `mount` call with `type` `cattle`, `src` `/w/h1`, `overlay` `{"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}`, `dst` `/d`, and `squash` `{"uid": 1000, "gid": 1000}`. "Mount pet h1" means a `mount` call with `type` `pet`, `src` `/w/h1`, `dst` `/d`, and `squash` `{"uid": 1000, "gid": 1000}`.

## Requirements

### Requirement: JSON-RPC API

The daemon SHALL listen on the `host:port` given at start. It SHALL accept JSON-RPC 2.0 requests at `POST /api`. The method `mount` SHALL take the params `type`, `src`, `overlay`, `dst`, and `squash`. `overlay` and `squash` MAY be `null`. An absent `overlay` SHALL mean `null`. `overlay.individual` and `overlay.shared` MAY each be `null` or absent, and an absent member SHALL mean `null`. A successful `mount` SHALL return the result `null`.

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

Before any change, `mount` SHALL return the JSON-RPC error `-32602` when a param is wrong. `overlay`, when present and not `null`, must be an object whose members `individual` and `shared` are each absent, `null`, or an object. `src`, `overlay.individual.src`, `overlay.shared.src`, and `dst` must be absolute paths of existing directories. Each `overlay.individual.directories` item must be a plain name, without `/`, and not `.` or `..`. `dst` must not be inside `src`, `overlay.individual.src`, or `overlay.shared.src`, and none of them may be inside `dst`. `dst` must not be inside another served `dst`, and must not contain one. The checks use resolved paths.

#### Scenario: Relative path
- **WHEN** a client calls `mount` with `src` `w/h1`
- **THEN** the response has the error code `-32602`
- **AND** `/d` does not change

#### Scenario: dst inside src
- **WHEN** a client calls `mount` with `src` `/w/h1` and `dst` `/w/h1/view`
- **THEN** the response has the error code `-32602`

#### Scenario: Name with a slash
- **WHEN** a client calls `mount` with `overlay.individual.directories` `["a/b"]`
- **THEN** the response has the error code `-32602`

#### Scenario: Overlay without members
- **WHEN** a client calls `mount` with `overlay` `{"src": "/w/overlay", "directories": [".idea"]}`
- **THEN** the response has the error code `-32602`

#### Scenario: Relative shared src
- **WHEN** a client calls `mount` with `overlay.shared` `{"src": "w/shared"}`
- **THEN** the response has the error code `-32602`

### Requirement: Foreign mounts

`mount` SHALL fail with a JSON-RPC error and change nothing when `dst`, or a path below it, holds a mount that this daemon does not serve.

#### Scenario: Old bindfs mount
- **GIVEN** a bindfs mount exists at `/d`
- **WHEN** a client calls mount h1
- **THEN** the response has an error that names the mount at `/d`
- **AND** the bindfs mount stays

### Requirement: Project list

The root of `dst` SHALL list the entries of `src`, the directories of `overlay.individual.src` when it is set, and the entries of `overlay.shared.src` when it is set. A directory of `overlay.individual.src` that `src` does not have SHALL show as a read-only project. A name that `overlay.shared.src` has SHALL not be a project.

#### Scenario: Union of projects
- **WHEN** a client calls mount h1
- **THEN** `/d` lists `proj1`, `proj2`, `proj3`, `notes.md`, and `tools`

#### Scenario: No overlay
- **WHEN** a client calls `mount` with `src` `/w/h1`, `overlay` `null`, and `dst` `/d`
- **THEN** `/d` lists `proj1` and `proj3` only

#### Scenario: Shared without individual
- **WHEN** a client calls `mount` with `src` `/w/h1`, `overlay` `{"shared": {"src": "/w/shared"}}`, and `dst` `/d`
- **THEN** `/d` lists `proj1`, `proj3`, `notes.md`, and `tools` only

#### Scenario: New project in src
- **GIVEN** h1 is mounted at `/d`
- **WHEN** an NFS client creates `/w/h1/proj4` with the file `main.py`
- **THEN** the next listing of `/d` contains `proj4`
- **AND** `/d/proj4` shows `main.py` and `.idea`

### Requirement: Passthrough of src

Every entry of `dst` that comes from `src` SHALL be a passthrough: reads, writes, creates, renames, and removes act on the matching path in `src`. This includes the root of `dst`.

#### Scenario: Write through the view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process writes `/d/proj1/main.py`
- **THEN** `/w/h1/proj1/main.py` has the new content

#### Scenario: New project through the view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process creates the directory `/d/proj5`
- **THEN** `/w/h1/proj5` exists

### Requirement: Source changes become visible

A change made directly in `src` SHALL be visible through `dst` within 1 second.

#### Scenario: NFS client edits a file
- **GIVEN** h1 is mounted at `/d`
- **WHEN** an NFS client writes new content to `/w/h1/proj1/main.py`
- **THEN** within 1 second a read of `/d/proj1/main.py` returns the new content

#### Scenario: NFS client renames an open file
- **GIVEN** h1 is mounted at `/d` and a process has `/d/proj1/a.py` open
- **WHEN** an NFS client renames `/w/h1/proj1/a.py` to `/w/h1/proj1/b.py`
- **THEN** within 1 second `/d/proj1/b.py` exists and `/d/proj1/a.py` does not
- **AND** the open file still reads the same content

### Requirement: Overlay directories

For each project `<p>` and each name `<n>` in `overlay.individual.directories`, `dst/<p>/<n>` SHALL show `overlay.individual.src/<p>/<n>`. Writes into it SHALL change `overlay.individual.src/<p>/<n>`. When `overlay.individual.src/<p>/<n>` does not exist, `dst/<p>/<n>` SHALL not exist. A create of `dst/<p>/<n>` SHALL create `overlay.individual.src/<p>/<n>`, and `overlay.individual.src/<p>` when it is missing. The daemon SHALL not create, read, or change `src/<p>/<n>`.

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

A project that exists only in `overlay.individual.src` SHALL contain only its overlay directories. Creates in it SHALL fail with `EROFS`, except creates of overlay directories. Removal or rename of such a project, or of an overlay directory entry `dst/<p>/<n>`, SHALL fail with `EPERM`.

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

### Requirement: In-place config change

A `mount` on a `dst` that the daemon already serves SHALL replace the config without an unmount. During the change, lookups of each project and overlay directory that exist in both configs SHALL never fail, and their inode numbers SHALL not change. After the call returns, the view SHALL show the new config. The daemon SHALL serve several `dst` paths at the same time.

#### Scenario: Change of host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** `/d/proj1` shows `/w/h2/proj1`
- **AND** `/d/proj2` shows `/w/h2/proj2`
- **AND** `/d/proj3` contains only `.idea`

#### Scenario: Concurrent lookups during a change
- **GIVEN** h1 is mounted at `/d` and a process calls `stat` on `/d/proj1/.idea` in a loop
- **WHEN** a client mounts `/w/h2` at `/d`
- **THEN** every `stat` call succeeds and returns the same inode number

#### Scenario: Shell inside a project
- **GIVEN** h1 is mounted at `/d` and a shell has its working directory in `/d/proj1`
- **WHEN** a client mounts `/w/h2` at `/d`
- **THEN** the call succeeds
- **AND** `ls` in the shell shows the entries of `/w/h2/proj1`

#### Scenario: Two views
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d2`
- **THEN** `/d` still shows h1 and `/d2` shows h2

### Requirement: Ownership squash

With `squash`, the view SHALL show every entry as owned by `squash.uid`:`squash.gid`, entries created through the view SHALL get on-disk owner 0:0, and a chown through the view SHALL never change the on-disk owner. Without `squash`, the view SHALL show on-disk owners, created entries SHALL get the caller's UID and GID, and chown SHALL change the on-disk owner.

#### Scenario: File with another owner
- **GIVEN** h1 is mounted at `/d` and `/w/h1/proj1/main.py` has on-disk owner 1001:1001 and mode `644`
- **WHEN** UID 1000 writes `/d/proj1/main.py`
- **THEN** the write succeeds

#### Scenario: File created with squash
- **GIVEN** h1 is mounted at `/d`
- **WHEN** UID 1000 creates `/d/proj1/new.py`
- **THEN** `/w/h1/proj1/new.py` has on-disk owner 0:0

#### Scenario: Root runs chown with squash
- **GIVEN** h1 is mounted at `/d`
- **WHEN** root runs `chown 1234:1234 /d/proj1/new.py`
- **THEN** the call succeeds
- **AND** `/w/h1/proj1/new.py` keeps on-disk owner 0:0

#### Scenario: File created without squash
- **GIVEN** `/w/h1` is mounted at `/d` with `squash` `null`
- **WHEN** UID 1000 with GID 1000 creates `/d/proj1/new.py`
- **THEN** `/w/h1/proj1/new.py` has on-disk owner 1000:1000

### Requirement: Data directory

The daemon SHALL require the start flag `--data-dir`. When the directory does not exist, the daemon SHALL create it, with missing parent directories. Without the flag, the daemon SHALL exit with a non-zero code.

#### Scenario: Missing data directory
- **GIVEN** `/var/lib/vm-herd-fs` does not exist
- **WHEN** the daemon starts with `--data-dir /var/lib/vm-herd-fs`
- **THEN** `/var/lib/vm-herd-fs` exists and the daemon serves the API

#### Scenario: No data directory flag
- **WHEN** the daemon starts with `--listen 127.0.0.1:8742` only
- **THEN** it exits with a non-zero code

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1` and a `mounts` array. Each item SHALL hold `type` and `dst`, with at most one item per `dst`. A `cattle` item SHALL also hold `overlay` and `squash`. A `pet` item SHALL also hold `src` and `squash`. The values SHALL have the form of the `mount` params. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "squash": {"uid": 1000, "gid": 1000}}]}`

#### Scenario: Pet mount
- **WHEN** a client calls mount pet h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "squash": {"uid": 1000, "gid": 1000}}]}`

### Requirement: State file updates

After each successful `cattle` mount with an `overlay`, `state.json` SHALL hold that call's cattle item for its `dst`. After each successful `cattle` mount without an `overlay`, it SHALL hold no item for its `dst`. After each successful `pet` mount, it SHALL hold that call's pet item for its `dst`. A failed `mount` SHALL not change the file.

#### Scenario: Change of host keeps one item
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** `state.json` holds one item for `/d`

#### Scenario: Mount without overlay
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h1` at `/d` with `overlay` `null`
- **THEN** `state.json` holds no item for `/d`

#### Scenario: Cattle without overlay replaces a pet
- **GIVEN** pet h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h1` at `/d` with `type` `cattle` and `overlay` `null`
- **THEN** `state.json` holds no item for `/d`

#### Scenario: Failed mount
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mount` with `src` `w/h1` and `dst` `/d2`
- **THEN** `state.json` does not change

### Requirement: State file version

When `state.json` does not exist, the daemon SHALL start with no restored views. When `state.json` has a `version` other than `1`, or is not valid, the daemon SHALL exit with a non-zero code and print the reason.

#### Scenario: Unknown version
- **GIVEN** `state.json` holds `{"version": 2, "mounts": []}`
- **WHEN** the daemon starts
- **THEN** it exits with a non-zero code and prints that version 2 is not supported

### Requirement: Restore at start

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. For a `cattle` item, the restored view SHALL list the directories of `overlay.individual.src` as projects and the entries of `overlay.shared.src` as shared entries. Each project SHALL follow the rules for a project that exists only in `overlay.individual.src`. Creates at the root of the view SHALL fail with `EROFS`. For a `pet` item, the restored view SHALL be the pet view of the stored `src`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

#### Scenario: Restart with an overlay
- **GIVEN** h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1`, `proj2`, `proj3`, `notes.md`, and `tools`
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea/workspace.xml` shows `/w/overlay/proj1/.idea/workspace.xml`
- **AND** `/d/notes.md` shows `/w/shared/notes.md`

#### Scenario: Restart with shared only
- **GIVEN** `/w/h1` was mounted at `/d` with `overlay` `{"shared": {"src": "/w/shared"}}` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `notes.md` and `tools` only
- **AND** a write to `/d/notes.md` changes `/w/shared/notes.md`
- **AND** `state.json` holds `"overlay": {"individual": null, "shared": {"src": "/w/shared"}}` for `/d`

#### Scenario: Restart with a pet
- **GIVEN** pet h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1` and `proj3` only
- **AND** a write to `/d/proj1/main.py` changes `/w/h1/proj1/main.py`
- **AND** `/d/proj1/main.py` shows owner 1000:1000

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

Only one daemon runs on a host. Before it restores a view, the daemon SHALL detach a `vm-herd-fs` mount at the item's `dst`, because an earlier run left it. When `dst` does not exist, when `dst` or a path below it holds another mount, or when a pet item's `src` is not a directory, the daemon SHALL skip the item, print the reason, and restore the other items. Skipped items SHALL stay in `state.json`.

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

### Requirement: Umount method

The daemon SHALL accept the JSON-RPC method `umount` with the param `dst`. `dst` must be an absolute path. A successful `umount` SHALL return the result `null` and SHALL not change `state.json`. When the daemon does not serve `dst`, or `dst` holds a pet view, `umount` SHALL fail with the JSON-RPC error `-32000` and change nothing.

#### Scenario: Dst not served
- **GIVEN** the daemon serves no view at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** the response has the error code `-32000`

#### Scenario: Pet view
- **GIVEN** pet h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** the response has the error code `-32000`
- **AND** `/d` still lists `proj1` and `proj3`
- **AND** `state.json` is the same as before the call

#### Scenario: Relative dst
- **WHEN** a client calls `umount` with `dst` `d`
- **THEN** the response has the error code `-32602`

#### Scenario: State file stays
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `state.json` is the same as before the call

### Requirement: Umount with a stored overlay

When `state.json` holds a cattle item for `dst`, `umount` SHALL replace the view in place with the view that a daemon start restores for that item. Lookups of each project and overlay directory that exist in both views SHALL never fail during the call, and their inode numbers SHALL not change.

#### Scenario: Back to the restored view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/d` lists `proj1`, `proj2`, `proj3`, `notes.md`, and `tools`
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea` has the same inode number as before the call

#### Scenario: Mount after umount
- **GIVEN** h1 is mounted at `/d` and a client called `umount` for `/d`
- **WHEN** a client calls mount h1
- **THEN** `/d/proj1` shows `/w/h1/proj1`

### Requirement: Umount without a stored overlay

When `state.json` holds no item for `dst`, `umount` SHALL unmount the view. Files that processes still hold open SHALL not block the call. After the call, `dst` SHALL be a plain directory, and a later `mount` for `dst` SHALL mount a new view.

#### Scenario: Plain directory again
- **GIVEN** `/w/h1` is mounted at `/d` with `overlay` `null`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/d` is not a mount point

#### Scenario: Open file during umount
- **GIVEN** `/w/h1` is mounted at `/d` with `overlay` `null` and a process holds `/d/proj1/main.py` open
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** the call succeeds and `/d` is not a mount point

#### Scenario: Mount again
- **GIVEN** `/w/h1` was mounted at `/d` with `overlay` `null`, and a client called `umount` for `/d`
- **WHEN** a client mounts `/w/h2` at `/d`
- **THEN** `/d/proj1` shows `/w/h2/proj1`

### Requirement: Mounts method

The daemon SHALL accept the JSON-RPC method `mounts` without params. The params MAY be absent, `null`, `{}`, or `[]`. Other params SHALL fail with the JSON-RPC error `-32602`. The result SHALL be an array with one item per `dst` that the daemon serves, sorted by `dst`.

#### Scenario: No views
- **GIVEN** the daemon serves no view
- **WHEN** a client calls `mounts`
- **THEN** the result is `[]`

#### Scenario: Unexpected params
- **WHEN** a client calls `mounts` with the params `{"dst": "/d"}`
- **THEN** the response has the error code `-32602`

### Requirement: Mounts items

Each `mounts` item SHALL hold `type`, `src`, `overlay`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored cattle view. A pet item SHALL hold its `src` and `overlay` `null`.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "cattle", "src": "/w/h1", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Pet after a restart
- **GIVEN** pet h1 was mounted at `/d` and the daemon restarted
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

### Requirement: Shared entries

For each entry `<e>` of `overlay.shared.src`, file or directory, `dst/<e>` SHALL show
`overlay.shared.src/<e>` as a passthrough: reads, writes, creates, renames, and removes below
`dst/<e>` act on `overlay.shared.src/<e>`. A shared entry SHALL hide a `src` entry of the same
name, project directories included. The daemon SHALL not read or change that `src` entry.

#### Scenario: Shared entries in the root
- **WHEN** a client calls mount h1
- **THEN** `/d` lists `proj1`, `proj2`, `proj3`, `notes.md`, and `tools`

#### Scenario: Write to a shared file
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process writes `/d/notes.md`
- **THEN** `/w/shared/notes.md` has the new content

#### Scenario: Shared file hides a src file
- **GIVEN** `/w/h1/notes.md` holds other content
- **WHEN** a client calls mount h1 and a process reads `/d/notes.md`
- **THEN** it reads the content of `/w/shared/notes.md`
- **AND** `/w/h1/notes.md` does not change

#### Scenario: Shared directory hides a project
- **GIVEN** `/w/shared/proj3` is a directory with the file `a.txt`
- **WHEN** a client calls mount h1 and a process lists `/d/proj3`
- **THEN** the list contains `a.txt` only
- **AND** the list does not contain `.idea`

#### Scenario: Remove inside a shared directory
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process removes `/d/tools/run.sh`
- **THEN** the call succeeds
- **AND** `/w/shared/tools/run.sh` does not exist

#### Scenario: Shared entry keeps its inode across a change of host
- **GIVEN** h1 is mounted at `/d` and a process calls `stat` on `/d/tools` in a loop
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** every `stat` call succeeds and returns the same inode number

### Requirement: Fixed shared entries

Removal or rename of `dst/<e>` for an entry `<e>` of `overlay.shared.src` SHALL fail with
`EPERM`. A rename onto `dst/<e>` SHALL fail with `EPERM`.

#### Scenario: Remove a shared file
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process removes `/d/notes.md`
- **THEN** the call fails with `EPERM`
- **AND** `/w/shared/notes.md` exists

#### Scenario: Rename a shared directory
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a process renames `/d/tools` to `/d/tools-old`
- **THEN** the call fails with `EPERM`

#### Scenario: Rename onto a shared file
- **GIVEN** h1 is mounted at `/d` and `/w/h1/README` exists
- **WHEN** a process renames `/d/README` to `/d/notes.md`
- **THEN** the call fails with `EPERM`

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
- **WHEN** a client calls `mount` with `type` `pet`, `src` `/w/h1`, `overlay` `{"shared": {"src": "/w/shared"}}`, and `dst` `/d`
- **THEN** the response has the error code `-32602`
- **AND** `/d` is not a mount point

### Requirement: Pet view

With `type` `pet`, `dst` SHALL show `src` as a passthrough with `squash` applied, with no overlay directories and no shared entries. The requirements Passthrough of src, Source changes become visible, Ownership squash, and In-place config change SHALL apply to a pet view.

#### Scenario: Pet lists src only
- **WHEN** a client calls mount pet h1
- **THEN** `/d` lists `proj1` and `proj3` only
- **AND** `/d/proj1` lists `main.py` only

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
