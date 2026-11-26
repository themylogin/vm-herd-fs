# Spec Delta

## ADDED Requirements

### Requirement: Dst creation

When `dst` does not exist, `mount` SHALL create it as a directory with mode `0755` after every param check passed. With `squash`, the new directory SHALL get on-disk owner `squash.uid:squash.gid`. Without `squash`, it SHALL keep owner 0:0. An existing `dst` SHALL not change. When the create fails, `mount` SHALL fail with the JSON-RPC error `-32000` and mount nothing.

#### Scenario: Missing dst with squash
- **GIVEN** `/d3` does not exist and `/` is a directory
- **WHEN** a client calls `mount` with `type` `cattle`, `src` `/w/h1`, `overlay` `null`, `dst` `/d3`, and `squash` `{"uid": 1000, "gid": 1000}`
- **THEN** the response has the result `null`
- **AND** `/d3` is a mount point that lists `proj1` and `proj3`
- **AND** after `umount` of `/d3`, `/d3` is a directory with on-disk owner 1000:1000 and mode `755`

#### Scenario: Missing dst without squash
- **GIVEN** `/d3` does not exist
- **WHEN** a client calls `mount` with `type` `cattle`, `src` `/w/h1`, `overlay` `null`, `dst` `/d3`, and `squash` `null`, then `umount` of `/d3`
- **THEN** `/d3` is a directory with on-disk owner 0:0

#### Scenario: Existing dst keeps its owner
- **GIVEN** `/d` exists with on-disk owner 1001:1001
- **WHEN** a client calls `mount` with `type` `cattle`, `src` `/w/h1`, `overlay` `null`, `dst` `/d`, and `squash` `{"uid": 1000, "gid": 1000}`, then `umount` of `/d`
- **THEN** `/d` has on-disk owner 1001:1001

#### Scenario: Failed check does not create dst
- **GIVEN** `/w/h1/view` does not exist
- **WHEN** a client calls `mount` with `src` `/w/h1` and `dst` `/w/h1/view`
- **THEN** the response has the error code `-32602`
- **AND** `/w/h1/view` does not exist

## MODIFIED Requirements

### Requirement: Param checks

Before any change, `mount` SHALL return the JSON-RPC error `-32602` when a param is wrong. `overlay`, when present and not `null`, must be an object whose members `individual` and `shared` are each absent, `null`, or an object. `src`, `overlay.individual.src`, and `overlay.shared.src` must be absolute paths of existing directories. `dst` must be an absolute path. When `dst` exists, it must be a directory. When `dst` does not exist, its parent must be an existing directory. Each `overlay.individual.directories` item must be a plain name, without `/`, and not `.` or `..`. `dst` must not be inside `src`, `overlay.individual.src`, or `overlay.shared.src`, and none of them may be inside `dst`. `dst` must not be inside another served `dst`, and must not contain one. The checks use resolved paths.

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
- **WHEN** a client calls `mount` with `overlay.individual.directories` `["a/b"]`
- **THEN** the response has the error code `-32602`

#### Scenario: Overlay without members
- **WHEN** a client calls `mount` with `overlay` `{"src": "/w/overlay", "directories": [".idea"]}`
- **THEN** the response has the error code `-32602`

#### Scenario: Relative shared src
- **WHEN** a client calls `mount` with `overlay.shared` `{"src": "w/shared"}`
- **THEN** the response has the error code `-32602`
