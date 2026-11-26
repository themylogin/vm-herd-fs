# Tasks

## 1. Mounts method

- [x] 1.1 Add `Shown` to `Mounted`, take it as a parameter in `attach`, and set it in `mount`, the restore, and `umount` as the design table says. Change `Registry.mounts` to a `BTreeMap`. Verify that the existing suite still passes.
- [x] 1.2 Add `MountsItem`, `Registry::mounts`, and the `mounts` method with the params check in `api.rs`. Verify with tests for "No views", "Unexpected params", "Mounted host", "Change of host", and "Two views".
- [x] 1.3 Verify the restored and unmounted cases with tests for "Restored view after a restart", "Restored view after umount", and "Unmounted view".

## 2. Docs

- [x] 2.1 Add the `mounts` method to `README.md` with a `curl` example and an example result. Verify that the documented `curl` example works against the daemon in the test container.
