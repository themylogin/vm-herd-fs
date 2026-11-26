# Tasks

## 1. Wire types and state shape

- [x] 1.1 In `src/api.rs`, add `MountType { Cattle, Pet }` with `rename_all = "lowercase"`, and a `type_` field with `rename = "type"` on `MountParams` and `MountsItem`. Verify: `tests/api.rs` has tests for a missing `type` and for `type` `sheep`, both returning `-32602`, and `dst` is not a mount point after each.
- [x] 1.2 In `src/state.rs`, replace `StoredMount` with the `type`-tagged enum `Cattle { dst, overlay, squash }` and `Pet { dst, src, squash }`, plus a `dst()` accessor. Verify: unit tests load a cattle item, load a pet item, reject an item without `type` with a message that names `type`, reject a cattle item with an unknown field, and round-trip both kinds through `save` and `load`.
- [x] 1.3 In `tests/common/mod.rs`, add `"type": "cattle"` to `Tree::params` and add `Tree::pet_params(host, dst)` with `type` `pet`, `src`, `dst`, and the same `squash`. Verify: `cargo test` compiles and every existing test still passes.

## 2. Registry

- [x] 2.1 In `src/mounts.rs`, branch `mount` on `params.type_`: for `pet`, fail with `-32602` when `overlay` is not `None`, build `Config { src, individual: None, shared: None, squash }`, and store a `Pet` item; for `cattle`, keep today's path with a `Cattle` item. Add `type_` to `Shown`. Verify: `tests/api.rs` has a test for a pet with `overlay` `{"shared": ...}` returning `-32602`; `tests/view.rs` has a test that a pet mount lists `proj1 proj3`, `/d/proj1` lists `main.py` only, and a write to `/d/proj1/main.py` reaches `h1/proj1/main.py`.
- [x] 2.2 Change `store` to take `Option<StoredMount>`. Verify: `tests/state.rs` asserts the pet item literal after a pet mount, and asserts no item for `/d` after a cattle mount with `overlay` `null` replaces a pet.
- [x] 2.3 Add a `tests/view.rs` test: a cattle h1 view at `/d`, then mount pet h1 at `/d`; the call succeeds, `/d` stays a mount point, `/d` lists `proj1 proj3` only. Verify: the test passes.
- [x] 2.4 Make `restore_item`, `restored_config`, and `restored_shown` match on the item; for `Pet`, require `src` to be a directory and open it as `Config.src`. Verify: `tests/state.rs` has a restart test for a pet that asserts the listing, a write through `/d/proj1/main.py`, owner 1000 on the file, and `mounts` returning the pet item with its `src`; and a test with a pet item whose `src` does not exist asserting `/d` is not a mount point, stderr names `/d`, and the item stays in `state.json`.
- [x] 2.5 In `umount`, look up the stored item first and fail with `-32000` for a `Pet`. Verify: `tests/umount.rs` asserts the error code for a pet `dst`, that `/d` still lists `proj1 proj3`, and that `state.json` did not change.
- [x] 2.6 Add `type` to every `mounts` literal in `tests/mounts.rs` and a test for a restored pet item `{"type": "pet", "src", "overlay": null, "dst", "squash"}`. Verify: the tests pass.

## 3. Docs

- [x] 3.1 Update `README.md`: `"type": "cattle"` in the `mount`, `mounts`, and `state.json` examples; a `type` row in the param table that defines `cattle` and `pet`, says `overlay` is not allowed for a pet, and that a pet `dst` has no `umount`; a pet example; the umount section with the pet error; the restart section with pet items and the skip rule for a missing `src`. Verify: every JSON literal in the README parses and matches the shape `tests/common/mod.rs` sends.
- [x] 3.2 Update the Purpose section of `openspec/specs/project-view/spec.md`: "mount h1" gains `type` `cattle`, and "mount pet h1" is defined as `type` `pet`, `src` `/w/h1`, `dst` `/d`, `squash` `{"uid": 1000, "gid": 1000}`. Verify: `openspec validate add-mount-type-pet --strict` passes.

## 4. Integration

- [x] 4.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
