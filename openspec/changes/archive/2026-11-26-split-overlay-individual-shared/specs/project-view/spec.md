# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: JSON-RPC API

The daemon SHALL listen on the `host:port` given at start. It SHALL accept JSON-RPC 2.0 requests at `POST /api`. The method `mount` SHALL take the params `src`, `overlay`, `dst`, and `squash`. `overlay` and `squash` MAY be `null`. An absent `overlay` SHALL mean `null`. `overlay.individual` and `overlay.shared` MAY each be `null` or absent, and an absent member SHALL mean `null`. A successful `mount` SHALL return the result `null`.

#### Scenario: Successful mount
- **WHEN** a client sends `POST /api` with the method `mount` and valid params
- **THEN** the response has the result `null`
- **AND** `dst` shows the view

#### Scenario: Overlay omitted
- **WHEN** a client calls `mount` with `src` `/w/h1`, `dst` `/d`, and `squash` `null`, and no `overlay` key
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

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1` and a `mounts` array. Each item SHALL hold `dst`, `overlay`, and `squash` in the form of the `mount` params, with at most one item per `dst`. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"dst": "/d", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "squash": {"uid": 1000, "gid": 1000}}]}`

### Requirement: Restore at start

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. A restored view SHALL list the directories of `overlay.individual.src` as projects and the entries of `overlay.shared.src` as shared entries. Each project SHALL follow the rules for a project that exists only in `overlay.individual.src`. Creates at the root of the view SHALL fail with `EROFS`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

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

#### Scenario: Create at the root of a restored view
- **GIVEN** the daemon restored the view at `/d`
- **WHEN** a process creates `/d/proj5`
- **THEN** the call fails with `EROFS`

#### Scenario: Mount after a restore
- **GIVEN** the daemon restored the view at `/d` and a process calls `stat` on `/d/proj1/.idea`
- **WHEN** a client calls mount h1
- **THEN** `/d/proj1` shows `/w/h1/proj1`
- **AND** `/d/proj1/.idea` has the same inode number as before the call

### Requirement: Mounts items

Each `mounts` item SHALL hold `src`, `overlay`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored view.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"src": "/w/h1", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

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
