# Tasks

## 1. Param and checks

- [x] 1.1 In `src/api.rs`, add `FallbackParams { src: PathBuf }` with `deny_unknown_fields`, and add `fallback: Option<FallbackParams>` to `MountParams` and `MountsItem`. In `src/mounts.rs`, add `fallback` to `Shown` and fill it from the params. Verify: `cargo build` passes, and a `mounts` call after mount h1 returns `"fallback": null`.
- [x] 1.2 In `src/mounts.rs` `mount`, reject a `pet` with a `fallback` that is not `null` with the message "fallback is not allowed for type pet". Check `fallback.src` with `existing_dir("fallback.src", ...)` and `check_apart(&dst, "fallback.src", ...)`. Verify: new `tests/api.rs` tests `missing_fallback_src` (code `-32602`, message contains `fallback.src`, no mount point), `fallback_with_an_unknown_member`, `dst_inside_the_fallback_store`, and `pet_with_a_fallback` (message contains `fallback`) pass.
- [x] 1.3 In `tests/common/mod.rs`, let `Tree::new` create `fallback/.git/HEAD` with `ref: refs/heads/main`. Add `Tree::fallback_params(host, dst)`, which is `params` plus `"fallback": {"src": <tree>/fallback}`. Verify: the existing tests still pass, because `params` does not send `fallback`.
- [x] 1.4 In `README.md`, add the `fallback` row to the param table and `fallback` to the cattle example. The `type` row says that a pet rejects `fallback`. Verify: the example JSON parses, and its keys match `fallback_params`.

## 2. Fallback entries in the view

- [x] 2.1 In `src/view.rs`, replace `Config.fallback_git: Option<PathBuf>` with `fallback: Option<Arc<OwnedFd>>`. Generalize `fallback_git_entry` to any name of the store: it is not an overlay name, `src/<p>` lacks it, and the store has it. In `project_entries`, add each such store entry with the realm of the project id. Verify: the six `view::tests` unit tests pass after a switch to a store directory, and a new unit test `any_store_entry` shows `.envrc` from the store in `proj2`.
- [x] 2.2 In `src/mounts.rs`, remove both hardcoded `/home/themylogin/test/.git` paths. Open `fallback.src` in `mount` with `open_dir`, and set `Config.fallback` from it. Verify: `grep -r themylogin/test src README.md` finds nothing.
- [x] 2.3 Add `tests/fallback.rs` with the scenarios of the requirement Fallback entries: `project_without_git`, `git_of_src_wins`, `overlay_name_wins`, `project_missing_in_src`, `new_entry_in_the_fallback_store`, `git_appears_in_src`, and `no_fallback`. Verify: the tests pass through FUSE.
- [x] 2.4 In `README.md`, replace the hardcoded-path paragraph with the fallback rule: which entries show, that `src` and overlay names win, and that the entries are read-only. Verify: `grep -n "test/.git" README.md` finds nothing.

## 3. Read-only access

- [x] 3.1 In `src/view.rs`, return `EROFS` from each mutating call when the node, its parent, or a rename or link target has a realm other than 0. The calls are `setattr`, `open` with `O_WRONLY`, `O_RDWR`, or `O_TRUNC`, `create`, `make`, `link`, `remove`, `rename`, `setxattr`, and `removexattr`. Also return `EROFS` for `remove` and `rename` of a top-level fallback entry, with the project as parent. Verify: unit tests `write_to_a_substitute_file` and `remove_a_substitute_entry` get `Errno::EROFS`, and the store keeps its files.
- [x] 3.2 Add the scenarios of the requirement Read-only fallback entries to `tests/fallback.rs`: `write_into_a_fallback_file` (as UID 1000), `create_inside_a_fallback_directory`, `remove_a_fallback_entry` (`rm -rf` exits non-zero, and `fallback/.git/HEAD` exists), `chmod_with_squash`, and `read_a_fallback_file`. Verify: the tests pass, and `fallback/.git` has the same files after each test.

## 4. Inode numbers

- [x] 4.1 Add the scenarios of the requirement Fallback inode numbers to `tests/fallback.rs`: `two_projects`, `change_of_host` (h1 to h2, both with `fallback_params`), and `back_to_the_restored_view` (after 5.2). Verify: the tests pass with no change in `src/nodes.rs`.

## 5. State and restore

- [x] 5.1 In `src/state.rs`, add `fallback: Option<FallbackParams>` to `StoredMount::Cattle`. In `src/mounts.rs`, fill it in the stored item. Verify: new unit tests load a cattle item without the key as `None`, and round-trip an item with `fallback`. `tests/state.rs` `mount_with_overlay` asserts `"fallback": null`, and a new test `mount_with_a_fallback` asserts the object.
- [x] 5.2 In `src/mounts.rs`, open the stored `fallback.src` in `restored_config`. An error there skips the item. Copy the stored `fallback` into `Shown` in `restored_shown`. Verify: new `tests/state.rs` tests `restart_with_a_fallback`, `cattle_item_without_fallback_key`, and `fallback_src_missing_at_start` (no mount point, a reason that names `/d`, and the item stays) pass.
- [x] 5.3 In `README.md`, add `fallback` to the cattle item of the `state.json` example. Add the missing `fallback.src` to the list of skip reasons at start. Verify: the example JSON parses with the same shape that `mount_with_a_fallback` asserts.

## 6. Mounts items

- [x] 6.1 Update the `tests/mounts.rs` literals with `"fallback": null`. Add `mounted_host_with_a_fallback`, and extend `restored_view_after_a_restart` with a stored `fallback`. Verify: the tests pass, and pet items show `"fallback": null`.
- [x] 6.2 In `README.md`, add `fallback` to the `mounts` example. Verify: the example matches the result that `mounted_host` asserts.

## 7. Integration

- [x] 7.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
- [x] 7.2 Build `dist/vm-herd-fs` with the README build command. Verify: `grep -a -c themylogin/test dist/vm-herd-fs` prints `0`.
