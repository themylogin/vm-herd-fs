# Tasks

## 1. Umount method

- [x] 1.1 Move the overlay-name check and the `overlay.src` open from `restore_item` into `restored_config`, and call it from `restore_item`. Verify that the existing suite still passes.
- [x] 1.2 Add `UmountParams` and the `umount` method in `api.rs`, and `Registry::umount` with the absolute-path check and the served check. Verify with tests for "Dst not served" and "Relative dst".
- [x] 1.3 For a `dst` with a stored item, replace the view in place with `restored_config`. Verify with tests for "Back to the restored view", "Mount after umount", and "State file stays".
- [x] 1.4 For a `dst` without a stored item, detach the mount with `MNT_DETACH`, remove the registry entry, and drop the session on a new thread. Verify with tests for "Plain directory again", "Open file during umount", and "Mount again".

## 2. Docs

- [x] 2.1 Add the `umount` method to `README.md`, with a `curl` example and its effect with and without a stored overlay. Verify that the documented `curl` example works against the daemon in the test container.
