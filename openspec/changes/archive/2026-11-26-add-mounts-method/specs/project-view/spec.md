# Spec Delta

## ADDED Requirements

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

Each `mounts` item SHALL hold `src`, `overlay`, `dst`, and `squash` in the form of the `mount` params, with the config that the view shows now. `dst` SHALL be the resolved path. `src` SHALL be `null` for a restored view.

#### Scenario: Mounted host
- **GIVEN** h1 is mounted at `/d`
- **WHEN** a client calls `mounts`
- **THEN** the result is `[{"src": "/w/h1", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "dst": "/d", "squash": {"uid": 1000, "gid": 1000}}]`

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
