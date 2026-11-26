# Design

## Context

See proposal.md for the motivation. `Config` in `view.rs` already covers a pet view: `src` set,
`individual` and `shared` unset. The `view.rs` code paths for such a config are the ones the
cattle view uses when `overlay` is `null`. The whole change lives in the wire types, the state
file, and the `Registry` in `mounts.rs`.

Today `StoredMount` is one struct with `overlay` as a required field, and `Registry::umount`
decides between "replace with the restored view" and "unmount" by the presence of a stored item.

The delta spec uses "mount pet h1" for a `mount` call with `type` `pet`, `src` `/w/h1`, `dst`
`/d`, and `squash` `{"uid": 1000, "gid": 1000}`. "Mount h1" gains `type` `cattle`. The main
spec's Purpose section must say both after archive; the task list carries that edit.

## Goals / Non-Goals

**Goals:**
- No change in `view.rs`.
- One state file shape that holds both item kinds without optional fields that one kind never
  uses.
- A pet view that comes back after a reboot exactly as it was mounted.

**Non-Goals:**
- A read of the old `state.json` without `type`.
- A cattle view that stores `src`.
- A way to unmount a pet view. The user excluded it.

## Decisions

### `type` is a serde enum on the wire

`api.rs` gets `MountType { Cattle, Pet }` with `rename_all = "lowercase"`. `MountParams` and
`MountsItem` get a `type_` field with `rename = "type"`. A missing key fails with serde's
"missing field `type`" and an unknown value with "unknown variant", both mapped to `-32602` by
the existing `mount` handler. No hand-written check.

### `StoredMount` is a `type`-tagged enum

```rust
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum StoredMount {
    Cattle { dst, overlay: OverlayParams, squash },
    Pet { dst, src: PathBuf, squash },
}
```

Each variant holds only the fields its kind uses. A `dst()` accessor serves the `BTreeMap` key
and the restore loop. The old file has no `type` and fails with "missing field `type`", which
`main` prints before it exits.

Alternative: one struct with `type`, `src: Option`, and `overlay: Option`. Rejected: it allows
a cattle item with `src` and a pet item with `overlay`, and needs a check in `load` to refuse
them.

### Pet checks and config in `Registry::mount`

`mount` matches on `params.type_` first. For `Pet`, `params.overlay.is_some()` fails with
`InvalidParams("overlay is not allowed for type pet")`. Then the shared checks run: `src` and
`dst` exist, `dst` is apart from `src` and from other served `dst` paths. The config is
`Config { src: Some(open_dir(&src)?), individual: None, shared: None, squash }`.

`store` takes the `StoredMount` to insert, or `None` to remove. `mount` builds `Some(Pet {..})`
for a pet, `Some(Cattle {..})` for a cattle with an overlay, and `None` otherwise. A type change
on a served `dst` goes through the same `replace` and `store` as a host change.

### Restore and umount match on the item

`restore_item` checks `dst` as today. For `Pet`, it also requires `src` to be a directory and
returns `Failed("src <path> is not a directory")`, which the restore loop prints and skips.
`restored_config` and `restored_shown` match on the variant: cattle gives `src: None` and the
overlay stores, pet gives `src: Some(open_dir(src)?)` and no stores. `Shown` gains `type_`.

`umount` looks up the stored item before anything else. A `Pet` item returns
`Failed("dst <path> is a pet view")`. A `Cattle` item replaces the view in place. No item
unmounts, as today.

### State file keeps version 1

Same reasoning as the overlay split: nothing is deployed, and `deny_unknown_fields` plus the
required tag make the old shape fail with a clear message.

## Risks / Trade-offs

- [`deny_unknown_fields` on an internally tagged enum] → serde accepts it for struct variants.
  The first task compiles the enum with a unit test for an unknown field before the rest of
  the work starts. If serde rejects the attribute, the fallback is an enum without it, and the
  unit test documents that an unknown field passes.
- [A pet `dst` cannot be freed through the API] → By request. A cattle `mount` with `overlay`
  `null` on the same `dst`, then `umount`, frees it. The README says so in one sentence.
- [Pet `src` on a dead NFS mount at start] → `open_dir` on it blocks or fails. The daemon
  prints the error and skips the item, like a missing `dst`.
