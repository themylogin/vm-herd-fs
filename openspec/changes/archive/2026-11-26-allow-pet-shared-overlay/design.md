# Design

## Context

See proposal.md for the motivation. `Config` in `view.rs` holds `src`, `individual`, `shared`,
and `squash` as independent options. A cattle view with `overlay` `{"shared": ...}` and no
`individual` already runs on `src` plus `shared`. A pet with shared entries is that same
`Config`. The view code needs no change.

Today the pet rules live in three places in `mounts.rs`: the check in `mount` that rejects any
`overlay`, the `StoredMount::Pet` item without `overlay`, and the `Pet` arms of
`restored_config` and `restored_shown` that set `shared: None` and `overlay: None`.

The delta spec uses "mount shared pet h1" for mount pet h1 with `overlay`
`{"shared": {"src": "/w/shared"}}`. The main spec's Purpose section must define it after
archive; the task list carries that edit.

## Goals / Non-Goals

**Goals:**
- No change in `view.rs` and `api.rs`.
- One code path for `overlay.shared` in `mount` and in restore, for both types.
- A shared pet view that comes back after a reboot exactly as it was mounted.

**Non-Goals:**
- `overlay.individual` on a pet. A pet has no projects.
- A change to `umount` for a pet. It still fails with `-32000`.

## Decisions

### The pet check narrows to `overlay.individual`

`mount` replaces `params.overlay.is_some()` with a check on
`params.overlay.as_ref().and_then(|overlay| overlay.individual.as_ref())`. The message is
"overlay.individual is not allowed for type pet". `overlay` `{}`, `{"individual": null}`, or
`{"shared": null}` passes for a pet, as `null` does, because an absent member means `null`.
The shared store then goes through the same `existing_dir`, `check_apart`, and `open_dir` steps
as for a cattle view. Nothing else in `mount` branches on the type.

Alternative: a separate `PetParams` type without `individual`. Rejected: `MountParams` is one
serde struct tagged by `type`, and a split would copy the shared checks.

### `StoredMount::Pet` holds `overlay: Option<OverlayParams>`

The pet item stores the `overlay` param as the client sent it, with `individual` serialized as
`null`. A pet mounted without `overlay` stores `"overlay": null`. Serde loads a missing key of
an `Option` field as `None`, so a pet item from an older `state.json` loads as a pet without
shared entries. The `squash` field already behaves this way.

Alternative: store `shared: Option<SharedParams>` on the pet item only. Rejected: the item
would differ from the `mount` params and from the `mounts` item, and `restored_shown` would
need to rebuild an `OverlayParams` from it.

Alternative: a `deserialize_with` function that makes the key required. Rejected: its only
effect is to refuse files that the daemon can read.

### Restore and `mounts` reuse the cattle shared path

`restored_config` opens the pet's `overlay.shared.src` with the same code as the cattle arm, so
the shared-store open moves to a small function both arms call. The pet arm returns
`Config { src: Some(open_dir(src)?), individual: None, shared, squash }`. A missing shared
store at start makes `open_dir` fail; `restore_item` prints the error and skips the item, as a
missing pet `src` does today. `restored_shown` for a pet copies the stored `overlay` into
`Shown.overlay`, so `mounts` shows `{"individual": null, "shared": {...}}` or `null`.

### Tests use `Tree::shared_pet_params`

Six tests in four files mount shared pet h1. `tests/common/mod.rs` gets
`Tree::shared_pet_params(host, dst)`, which is `pet_params` with `overlay`
`{"shared": {"src": <shared>}}`.

## Risks / Trade-offs

- [A shared entry hides a pet `src` entry of the same name] → Same rule as for a cattle view,
  where it hides a project. The delta spec has a scenario for it, and the README `overlay.shared`
  row already says so.
