# Spec Delta

## MODIFIED Requirements

### Requirement: Mount type

The `mount` param `type` SHALL be `cattle` or `pet`. A missing `type` or another value SHALL fail with the JSON-RPC error `-32602` before any change. With `type` `pet`, `overlay.individual` MUST be absent or `null`; otherwise `mount` SHALL fail with `-32602`. `overlay.shared` MAY be set for both types.

#### Scenario: Missing type
- **WHEN** a client calls `mount` with `src` `/w/h1`, `dst` `/d`, and no `type` key
- **THEN** the response has the error code `-32602`
- **AND** `/d` is not a mount point

#### Scenario: Unknown type
- **WHEN** a client calls `mount` with `type` `sheep`
- **THEN** the response has the error code `-32602`

#### Scenario: Pet with an overlay
- **WHEN** a client calls `mount` with `type` `pet`, `src` `/w/h1`, `overlay` `{"individual": {"src": "/w/overlay", "directories": [".idea"]}}`, and `dst` `/d`
- **THEN** the response has the error code `-32602`
- **AND** the error message names `overlay.individual`
- **AND** `/d` is not a mount point

#### Scenario: Pet with a shared overlay
- **WHEN** a client calls mount shared pet h1
- **THEN** the response has the result `null`
- **AND** `/d` is a mount point

### Requirement: Pet view

With `type` `pet`, `dst` SHALL show `src` as a passthrough with `squash` applied, with no overlay directories. When `overlay.shared` is set, the root of `dst` SHALL also show the entries of `overlay.shared.src`. The requirements Passthrough of src, Source changes become visible, Ownership squash, In-place config change, Shared entries, and Fixed shared entries SHALL apply to a pet view.

#### Scenario: Pet lists src only
- **WHEN** a client calls mount pet h1
- **THEN** `/d` lists `proj1` and `proj3` only
- **AND** `/d/proj1` lists `main.py` only

#### Scenario: Pet lists src and shared entries
- **WHEN** a client calls mount shared pet h1
- **THEN** `/d` lists `notes.md`, `proj1`, `proj3`, and `tools` only
- **AND** `/d/proj1` lists `main.py` only

#### Scenario: Write to a shared file in a pet view
- **GIVEN** shared pet h1 is mounted at `/d`
- **WHEN** UID 1000 writes `/d/notes.md`
- **THEN** `/w/shared/notes.md` has the new content
- **AND** `/w/h1/notes.md` does not exist

#### Scenario: Shared directory hides a pet src directory
- **GIVEN** `/w/shared/proj3` is a directory with the file `a.txt`
- **WHEN** a client calls mount shared pet h1
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

### Requirement: State file format

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1` and a `mounts` array. Each item SHALL hold `type` and `dst`, with at most one item per `dst`. A `cattle` item SHALL also hold `overlay` and `squash`. A `pet` item SHALL also hold `src`, `overlay`, and `squash`. The values SHALL have the form of the `mount` params. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "squash": {"uid": 1000, "gid": 1000}}]}`

#### Scenario: Pet mount
- **WHEN** a client calls mount pet h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "overlay": null, "squash": {"uid": 1000, "gid": 1000}}]}`

#### Scenario: Pet mount with a shared overlay
- **WHEN** a client calls mount shared pet h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "overlay": {"individual": null, "shared": {"src": "/w/shared"}}, "squash": {"uid": 1000, "gid": 1000}}]}`

#### Scenario: Pet item without overlay key
- **GIVEN** `state.json` holds `{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "squash": null}]}`
- **WHEN** the daemon starts
- **THEN** `/d` lists `proj1` and `proj3` only

### Requirement: Restore at start

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. For a `cattle` item, the restored view SHALL list the directories of `overlay.individual.src` as projects and the entries of `overlay.shared.src` as shared entries. Each project SHALL follow the rules for a project that exists only in `overlay.individual.src`. Creates at the root of the view SHALL fail with `EROFS`. For a `pet` item, the restored view SHALL be the pet view of the stored `src` and the stored `overlay`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

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

#### Scenario: Restart with a shared pet
- **GIVEN** shared pet h1 was mounted at `/d` and the daemon stopped
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

### Requirement: Mounts items

Each `mounts` item SHALL hold `type`, `src`, `overlay`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored cattle view. A pet item SHALL hold its `src` and its `overlay`, with `individual` `null` when `overlay` is not `null`.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "cattle", "src": "/w/h1", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Pet after a restart
- **GIVEN** pet h1 was mounted at `/d` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": null, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

#### Scenario: Shared pet after a restart
- **GIVEN** shared pet h1 was mounted at `/d` and the daemon restarted
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"type": "pet", "src": "/w/h1", "overlay": {"individual": null, "shared": {"src": "/w/shared"}}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

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
