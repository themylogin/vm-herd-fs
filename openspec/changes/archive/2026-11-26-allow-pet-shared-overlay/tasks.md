# Tasks

## 1. State shape

- [x] 1.1 In `src/state.rs`, add `overlay: Option<OverlayParams>` to `StoredMount::Pet`. Verify: unit tests load a pet item with `"overlay": {"individual": null, "shared": {"src": "/w/shared"}}`, load a pet item with `"overlay": null`, load a pet item without the `overlay` key with `overlay` `None`, and round-trip a shared pet item through `save` and `load`.
- [x] 1.2 In `src/mounts.rs`, fill `overlay: params.overlay` when `mount` builds the `Pet` item. Verify: `tests/state.rs` `pet_mount` asserts `"overlay": null` in the item; a new test `pet_mount_with_a_shared_overlay` asserts the item with `"overlay": {"individual": null, "shared": {"src": <shared>}}`.

## 2. Mount check and view

- [x] 2.1 In `src/mounts.rs`, change the pet check in `mount` to reject only `overlay.individual` with the message "overlay.individual is not allowed for type pet". Verify: `tests/api.rs` `pet_with_an_overlay` sends `overlay` `{"individual": {...}}`, gets `-32602`, a message that contains `overlay.individual`, and no mount point; a new test `pet_with_a_shared_overlay` gets the result `null` and a mount point.
- [x] 2.2 Add `tests/view.rs` tests for a shared pet: `pet_lists_src_and_shared_entries` asserts `/d` lists `notes.md proj1 proj3 tools` and `/d/proj1` lists `main.py` only; `write_to_a_shared_file_in_a_pet_view` writes `/d/notes.md` as UID 1000 and asserts `shared/notes.md` has the content and `h1/notes.md` does not exist; `shared_directory_hides_a_pet_src_directory` creates `shared/proj3/a.txt` and asserts `/d/proj3` lists `a.txt` only. Verify: the tests pass with the check from 2.1 and no change in `view.rs`.

## 3. Restore and mounts

- [x] 3.1 In `src/mounts.rs`, move the shared-store open of `restored_config` into a function both arms call, open the pet's stored `overlay.shared.src` in the `Pet` arm, and copy the stored `overlay` into `Shown.overlay` in `restored_shown`. Verify: `tests/state.rs` `restart_with_a_shared_pet` mounts shared pet h1, restarts, asserts `/d` lists `notes.md proj1 proj3 tools`, a write to `/d/notes.md` reaches `shared/notes.md`, and a write to `/d/proj1/main.py` reaches `h1/proj1/main.py`.
- [x] 3.2 Add `tests/mounts.rs` `shared_pet_after_a_restart`: mount shared pet h1, restart, assert the item `{"type": "pet", "src": <h1>, "overlay": {"individual": null, "shared": {"src": <shared>}}, "dst": <d>, "squash": {"uid": 1000, "gid": 1000}}`. Verify: the test passes and `pet_after_a_restart` still asserts `"overlay": null`.

## 4. Docs

- [x] 4.1 Update `README.md`: the `type` row says a pet accepts `overlay.shared` and rejects `overlay.individual`; the pet example gains `"overlay": {"shared": {...}}`; the `state.json` example's pet item gains `overlay`; the `mounts` paragraph says a pet item shows its `overlay`. Verify: every JSON literal in the README parses and matches the shape `tests/common/mod.rs` sends.
- [x] 4.2 Update the Purpose section of `openspec/specs/project-view/spec.md`: define "mount shared pet h1" as mount pet h1 with `overlay` `{"shared": {"src": "/w/shared"}}`. Verify: `openspec validate allow-pet-shared-overlay --strict` passes.

## 5. Integration

- [x] 5.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
