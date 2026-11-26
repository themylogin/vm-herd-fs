# Tasks

## 1. Params and state shape

- [x] 1.1 In `src/api.rs`, replace `OverlayParams` with `{individual: Option<IndividualParams>, shared: Option<SharedParams>}`, all three with `deny_unknown_fields`. Verify: in `tests/api.rs`, `mount` with `overlay` `{"src": ..., "directories": [...]}` returns `-32602`, a `mount` without the `overlay` key succeeds, and `overlay` `{"shared": {...}}` succeeds.
- [x] 1.2 In `src/state.rs`, keep `StoredMount.overlay` on the new type and update the unit tests' JSON literals to the new shape. Verify: `cargo test state` passes, and an old-shape file makes `load` return an error.
- [x] 1.3 In `tests/common/mod.rs`, add `shared/notes.md` and `shared/tools/run.sh` to `Tree::new`, and make `Tree::params` send `overlay` `{"individual": {...}, "shared": {"src": .../shared}}`. Verify: `cargo test` compiles; existing tests that assert root listings fail only on the two new names.

## 2. Mount checks and config construction

- [x] 2.1 In `src/mounts.rs`, check `overlay.individual.src` and `overlay.shared.src` with `existing_dir` and `check_apart`, rename the `check_name` message to `overlay.individual.directories item`, and build `Config { individual, shared }` for `mount` and `restored_config`. Verify: `tests/api.rs` has a test for `overlay.shared` `{"src": "w/shared"}` returning `-32602`, and the slash test asserts the new message.
- [x] 2.2 Update `Shown`, `mounts()`, and `store()` for the new `OverlayParams`. Verify: `tests/mounts.rs` asserts the full `overlay` object in the new shape for a mounted host and a restored view.

## 3. Shared entries in the view

- [x] 3.1 In `src/view.rs`, flatten `Config` to `individual: Option<Individual>` and `shared: Option<Shared>`; rename `Overlay` to `Individual`. Verify: `cargo clippy --all-targets -- -D warnings` is clean.
- [x] 3.2 Add `Config::shared_entry(name) -> Res<Option<OwnedFd>>` and make `is_project` return `false` for a shared name. In `lookup` for `Node::Root`, try the shared entry first, then the project, then the `src` entry. Verify: `tests/view.rs` reads `/d/notes.md` and gets the shared content while `h1/notes.md` holds other content and stays unchanged.
- [x] 3.3 In `root_entries`, list `shared.src` first and seed the `names` set, so `src` entries and individual-only projects with the same name are skipped. Verify: `tests/view.rs` asserts `/d` lists `proj1 proj2 proj3 notes.md tools`, and a `shared/proj3` directory hides the project and its `.idea`.
- [x] 3.4 Extend `check_fixed(Root, name)` with the shared name check. Verify: `tests/view.rs` asserts `EPERM` for `unlink /d/notes.md`, `rename /d/tools`, and `rename /d/README -> /d/notes.md`, and success for `unlink /d/tools/run.sh`.
- [x] 3.5 Make `root_stat` and `statfs` fall back to `shared.src` when `src` and `individual` are `None`. Verify: a `mount` with `overlay` `{"individual": null, "shared": {...}}` followed by a restart lists `notes.md tools` only and `stat /d` succeeds.

## 4. Restore, switch, and docs

- [x] 4.1 Add a `tests/state.rs` case: restart with the full overlay lists `proj1 proj2 proj3 notes.md tools`, `/d/notes.md` shows `shared/notes.md`, and a root create fails with `EROFS`. Verify: the test passes.
- [x] 4.2 Add a `tests/switch.rs` case: `stat /d/tools` in a loop keeps its inode across a mount of `h2`. Verify: the test passes.
- [x] 4.3 Update `README.md`: the `mount` example, the param table rows for `overlay`, `overlay.individual.*`, `overlay.shared.src`, the `mounts` example, and the `state.json` example. State the shadow rule in one sentence, and that `overlay`, `individual`, and `shared` default to `null` when absent. Verify: every JSON literal in the README parses and matches the shape `tests/common/mod.rs` sends.
- [x] 4.4 Update the Purpose section of `openspec/specs/project-view/spec.md`: add `/w/shared` with `notes.md` and `tools/run.sh` to the example paths, and give "mount h1" the new `overlay` value. Verify: `openspec validate --strict` passes for the change.

## 5. Integration

- [x] 5.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
