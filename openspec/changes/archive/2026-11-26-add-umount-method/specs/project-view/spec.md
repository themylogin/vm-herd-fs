# Spec Delta

## ADDED Requirements

### Requirement: Umount method

The daemon SHALL accept the JSON-RPC method `umount` with the param `dst`. `dst` must be an absolute path. A successful `umount` SHALL return the result `null` and SHALL not change `state.json`. When the daemon does not serve `dst`, `umount` SHALL fail with the JSON-RPC error `-32000` and change nothing.

#### Scenario: Dst not served
- **GIVEN** the daemon serves no view at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** the response has the error code `-32000`

#### Scenario: Relative dst
- **WHEN** a client calls `umount` with `dst` `d`
- **THEN** the response has the error code `-32602`

#### Scenario: State file stays
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `state.json` is the same as before the call

### Requirement: Umount with a stored overlay

When `state.json` holds an item for `dst`, `umount` SHALL replace the view in place with the view that a daemon start restores for that item. Lookups of each project and overlay directory that exist in both views SHALL never fail during the call, and their inode numbers SHALL not change.

#### Scenario: Back to the restored view
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `umount` with `dst` `/d`
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3`
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
