# Tasks

## 1. Wire types and state

- [x] 1.1 In `src/api.rs`, make `OverlayParams` `{src, directories}` and delete `IndividualParams` and `SharedParams`. In `src/mounts.rs`, check that a pet has no `overlay` with the message "overlay is not allowed for type pet". Verify: `tests/api.rs` has tests for an old `{"individual": ...}` overlay (`-32602`), a pet with a flat overlay (`-32602`, message names `overlay`), and a pet with `{"shared": ...}` (`-32602`, no mount point); `Tree::params` sends the flat overlay and `Tree::shared_pet_params` is gone.
- [x] 1.2 In `src/state.rs`, add `binds: Vec<StoredBind { src, shared }>` to `State`, drop `overlay` from `StoredMount::Pet`, and make `save` take both lists. Verify: unit tests load a cattle item with the flat overlay, load a pet item without `overlay`, reject a pet item with `overlay`, reject a file without `binds`, and round-trip `mounts` and `binds`; `tests/state.rs` asserts `"binds": []` in the "Mount with overlay" and "Pet mount" literals.

## 2. View without a shared store

- [x] 2.1 In `src/view.rs`, delete `Shared`, `Config.shared`, `shared_entry`, `shared_has`, the shared merge in the root listing, and the shared `EPERM` checks; rename `Individual` to `Overlay` and `Config.individual` to `Config.overlay`, and follow the names in `src/mounts.rs`. Verify: `grep -rn 'shared\|individual' src/view.rs` finds nothing, and `check` exits 0.
- [x] 2.2 Rework the tests that used the shared store: delete the `tests/view.rs` tests for "Fixed shared entries" and the shared-entry tests that `tests/binds.rs` now covers; move "Shared without individual", "Pet lists src and shared entries", "Write to a shared file in a pet view", and "Shared directory hides a pet src directory" to a bound `h1`; update "Union of projects", "Restart with an overlay", "Back to the restored view", and the `mounts` literals to the new listings and the flat overlay; move the inode test from `tests/switch.rs` to `tests/binds.rs`. Verify: `test` exits 0 for `tests/view.rs`, `tests/switch.rs`, `tests/umount.rs`, `tests/mounts.rs`, and `tests/state.rs`.

## 3. Bind and unbind

- [x] 3.1 In `src/mounts.rs`, remove `binds` from `Mounted`, the bind calls from `mount` and `umount`, and `detach_earlier_binds`; add `Registry.binds` and `Registry::bind` with the inode presence check and the four steps of the design. In `src/api.rs`, add the method `bind`. Verify: `tests/binds.rs` has a test for each scenario of "Bind method", "Bind again", and "Bind mount points", and `tests/api.rs` has "Relative shared src" for `bind`.
- [x] 3.2 Add `Registry::unbind` and the method `unbind`. Verify: `tests/binds.rs` has a test for each scenario of "Unbind method".
- [x] 3.3 Write `binds` to `state.json` after each successful `bind` and `unbind`. Verify: `tests/binds.rs` has "Bind is stored" and "Unbind is stored", and a failed `bind` leaves `state.json` unchanged.
- [x] 3.4 In `Registry::restore`, bind each stored item before the views, and print and skip a failed item. Verify: `tests/binds.rs` has the three scenarios of "Binds at start"; "Restart after the bind mounts went away" kills the daemon, detaches the bind mounts with `umount -l`, and starts the daemon; "Restart with shared only" and "Restart with a shared pet" in `tests/state.rs` use a bound `h1`.
- [x] 3.5 Add `tests/binds.rs` tests for the four scenarios of "Binds and views are independent". Verify: the tests pass.

## 4. Docs

- [x] 4.1 Update `README.md`: the `mount` example and table with the flat `overlay`; the `bind` and `unbind` methods with examples, errors, and the rule to call `unbind` before a delete of `src`; the "Shared entries in src" section rewritten for `bind`; `-32601` with the method list; `mounts` and `state.json` examples with the flat overlay and `binds`; the restart section with binds at start; a note to remove an old `state.json` once. Verify: every JSON literal in the README parses, `grep -n 'individual\|overlay.shared' README.md` finds nothing, and the README names `crossmnt` and `unbind`.
- [x] 4.2 Update the Purpose section of `openspec/specs/project-view/spec.md`: "mount h1" uses `overlay` `{"src": "/w/overlay", "directories": [".idea"]}`, "bind h1" means a `bind` call with `src` `/w/h1` and `shared` `/w/shared`, and "mount shared pet h1" goes away. Verify: `openspec validate add-bind-method --strict` passes.

## 5. Integration

- [x] 5.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`, and run `grep -rn 'individual\|shared_pet\|overlay.shared' src tests`. Verify: both commands exit 0, and the grep finds only tests that send an old shape and expect `-32602`, plus scenario names that the spec keeps.

## Workflow follow-up

- Delete the superseded change `openspec/changes/bind-shared-entries-into-src/` after the user confirms.
