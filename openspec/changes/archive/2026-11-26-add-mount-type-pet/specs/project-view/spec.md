# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
