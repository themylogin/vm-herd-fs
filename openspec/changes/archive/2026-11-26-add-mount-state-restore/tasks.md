# Tasks

## 1. Data directory and test helper

- [x] 1.1 Add the required `--data-dir` flag and create the directory with `create_dir_all` at start. Change the test daemon helper to take a data directory, to restart on the same directory, and to detach every mounted or restored `dst` at teardown. Update the existing tests to pass a temporary data directory. Verify with tests for "Missing data directory" and "No data directory flag", and verify that the existing suite still passes.

## 2. State file

- [x] 2.1 Add `src/state.rs` with the `State` and `StoredMount` types, the load with the version check, and the write through `state.json.tmp`, `fsync`, and rename. Verify with unit tests for a missing file, version 1, version 2, and invalid JSON, and with a test for "Unknown version" that starts the daemon.
- [x] 2.2 Keep the items in the registry and write the file after each successful `mount`: set the item with `overlay`, remove it without `overlay`. Return `-32000` when the write fails. Verify with tests for "Mount with overlay", "Change of host keeps one item", "Mount without overlay", and "Failed mount".

## 3. Restored views

- [x] 3.1 Make `Config.src` optional and apply the rules from the design for a view without `src`: root attributes and listing from `overlay.src`, `EROFS` for root creates and root metadata changes, and `statfs` on `overlay.src`. Move the session setup into one function shared by `mount` and the restore. Verify that the existing suite still passes.
- [x] 3.2 Add the mount source to `parse_mountinfo`, the check for a `vm-herd-fs` mount at `dst`, and the detach through `rustix::mount::unmount` with `MNT_DETACH`. Verify with a unit test for the source field of a mountinfo line.
- [x] 3.3 Add `Registry::restore` and call it in `main` before `api::serve`. Skip and print items whose `dst` is missing, holds a mount, or whose `overlay.src` is missing. Verify with tests for "Restart with an overlay", "Create at the root of a restored view", "Mount after a restore", "Restart after a crash", and "Foreign mount at a stored dst".

## 4. Docs

- [x] 4.1 Update `README.md`: the start command with `--data-dir`, the state file format, the restore at start, and the new stop section. Verify that the documented start and `curl` commands run as written in the test container.
