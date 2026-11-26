# Spec Delta

## ADDED Requirements

### Requirement: Binds method

The daemon SHALL accept the JSON-RPC method `binds` without params. The params MAY be absent, `null`, `{}`, or `[]`. Other params SHALL fail with the JSON-RPC error `-32602`. The result SHALL be an array with one item `{"src", "shared"}` per `binds` item of `state.json`, sorted by `src`, with the paths of that item. An item whose bind failed at start SHALL be in the result.

#### Scenario: No binds
- **GIVEN** no client called `bind`
- **WHEN** a client calls `binds`
- **THEN** the result is `[]`

#### Scenario: Bound host
- **GIVEN** a client called bind h1
- **WHEN** a client calls `binds`
- **THEN** the result is `[{"src": "/w/h1", "shared": "/w/shared"}]`

#### Scenario: Two hosts
- **GIVEN** a client called `bind` with `src` `/w/h2` and `shared` `/w/shared`, and then bind h1
- **WHEN** a client calls `binds`
- **THEN** the result has the item for `/w/h1` first and the item for `/w/h2` second

#### Scenario: After unbind
- **GIVEN** a client called bind h1 and then `unbind` with `src` `/w/h1`
- **WHEN** a client calls `binds`
- **THEN** the result is `[]`

#### Scenario: Item that failed at start
- **GIVEN** `state.json` holds `"binds": [{"src": "/w/h1", "shared": "/w/shared"}]` and `/w/h1/tools` is a file
- **WHEN** the daemon starts and a client calls `binds`
- **THEN** the result is `[{"src": "/w/h1", "shared": "/w/shared"}]`

#### Scenario: Unexpected params
- **WHEN** a client calls `binds` with the params `{"src": "/w/h1"}`
- **THEN** the response has the error code `-32602`
