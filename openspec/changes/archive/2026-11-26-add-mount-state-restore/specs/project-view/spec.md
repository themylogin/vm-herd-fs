# Spec Delta

## ADDED Requirements

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

The daemon SHALL keep `<data-dir>/state.json` as a JSON object with `version` set to `1` and a `mounts` array. Each item SHALL hold `dst`, `overlay`, and `squash` in the form of the `mount` params, with at most one item per `dst`. The file SHALL never be partly written.

#### Scenario: Mount with overlay
- **WHEN** a client calls mount h1
- **THEN** `state.json` holds `{"version": 1, "mounts": [{"dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "squash": {"uid": 1000, "gid": 1000}}]}`

### Requirement: State file updates

After each successful `mount` with an `overlay`, `state.json` SHALL hold that call's item for its `dst`. After each successful `mount` without an `overlay`, it SHALL hold no item for its `dst`. A failed `mount` SHALL not change the file.

#### Scenario: Change of host keeps one item
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h2` at `/d` with the same overlay and squash
- **THEN** `state.json` holds one item for `/d`

#### Scenario: Mount without overlay
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client mounts `/w/h1` at `/d` with `overlay` `null`
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

Before it serves the API, the daemon SHALL mount a restored view at the `dst` of each item in `state.json`. A restored view SHALL list the directories of `overlay.src` as projects. Each project SHALL follow the rules for a project that exists only in `overlay.src`. Creates at the root of the view SHALL fail with `EROFS`. The item's `squash` SHALL apply. A later `mount` for that `dst` SHALL replace the config in place.

#### Scenario: Restart with an overlay
- **GIVEN** h1 was mounted at `/d` and the daemon stopped
- **WHEN** the daemon starts again
- **THEN** `/d` lists `proj1`, `proj2`, and `proj3`
- **AND** `/d/proj1` contains only `.idea`
- **AND** `/d/proj1/.idea/workspace.xml` shows `/w/overlay/proj1/.idea/workspace.xml`

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

Only one daemon runs on a host. Before it restores a view, the daemon SHALL detach a `vm-herd-fs` mount at the item's `dst`, because an earlier run left it. When `dst` does not exist, or `dst` or a path below it holds another mount, the daemon SHALL skip the item, print the reason, and restore the other items. Skipped items SHALL stay in `state.json`.

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
