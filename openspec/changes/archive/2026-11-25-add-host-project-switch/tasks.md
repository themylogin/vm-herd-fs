# Tasks

## 1. Project setup

- [x] 1.1 Create the Cargo crate `vm-herd-fs` with the dependencies `fuser`, `rustix`, `libc`, `clap`, `serde`, `serde_json`, and `tiny_http`, and the dev dependency `ureq`. Add the `--listen <host:port>` argument. Verify that `vm-herd-fs --help` runs in the container.
- [x] 1.2 Add a `Dockerfile` (Rust slim) and `compose.yaml` with a `check` profile (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`) and a `test` profile (`cargo test` with `/dev/fuse` and `SYS_ADMIN`). Verify that both profiles pass on the skeleton.

## 2. API and mount lifecycle

- [x] 2.1 Add the `tiny_http` server for `POST /api`, the JSON-RPC 2.0 envelope, and the `mount` method in `api.rs` with typed params. Return HTTP 404 and 405 for other paths and methods. Add a test helper that starts the daemon on a free port and calls `mount`. Verify with a test for "Unknown method" and a parse-error test.
- [x] 2.2 Add the param checks on resolved paths: absolute existing directories, plain names in `directories`, no overlap between `dst`, `src`, `overlay.src`, and other served `dst` paths. Verify with tests for "Relative path", "dst inside src", and "Name with a slash".
- [x] 2.3 Add the first mount: the mountinfo check for foreign mounts at or below `dst`, then a `fuser` background session with the mount options from the design. Raise `RLIMIT_NOFILE` at start. Verify with tests for "Successful mount" (an empty `src`) and "Old bindfs mount" (a tmpfs at `dst`).

## 3. Node table and read path

- [x] 3.1 Add the node table: `O_PATH` descriptors keyed by `(st_dev, st_ino)`, node IDs that are never reused, and `forget` handling. Verify with unit tests: a second lookup of the same file returns the same ID, and an ID stays the same after `forget` and a new lookup.
- [x] 3.2 Add the virtual root and project nodes and the root listing from `src` and `overlay.src`. Verify with tests for "Union of projects", "No overlay", and "New project in src".
- [x] 3.3 Add `lookup`, `getattr`, `readdir`, `readlink`, `open`, `read`, `release`, `statfs`, and the xattr reads for real nodes, with a 1-second TTL and a TTL of 0 for negative entries. Verify with tests for "NFS client edits a file", "NFS client renames an open file", and a symlink read.
- [x] 3.4 Add the overlay directories: the lookup by name, the `readdir` merge, and the creation of missing `overlay.src/<p>/<n>`. Verify with tests for "Source has its own .idea", "Source has no .idea", and "New project gets an overlay directory".

## 4. Write path and ownership

- [x] 4.1 Add `create`, `write`, `flush`, `fsync`, `mkdir`, `mknod`, `symlink`, `link`, `unlink`, `rmdir`, `rename` (with flags), `fallocate`, and the xattr writes, with the request `umask`. Verify with tests for "Write through the view", "New project through the view", and a rename inside a project.
- [x] 4.2 Add the squash rules and the non-squash rules for attribute replies, created entries, and `setattr`. Verify with tests for "File with another owner", "File created with squash", "Root runs chown with squash", and "File created without squash".
- [x] 4.3 Add the read-only project rules and the fixed overlay entries. Verify with tests for "Project missing in src", "Write into a read-only project", and "Remove an overlay directory".

## 5. In-place config change

- [x] 5.1 Add the per-mount config lock, the recording of looked-up names under the root and project nodes, and the swap with `inval_entry` and `inval_inode` sent from the API thread. Verify with tests for "Change of host", "Shell inside a project", and "Two views".
- [x] 5.2 Verify "Concurrent lookups during a change" with a test: a thread runs `stat` on `/d/proj1/.idea` in a loop and sees no failure and one inode number across 50 config changes. A second thread keeps creating files in `/d/proj1` during the changes, so the test also covers the notification deadlock.
- [x] 5.3 Write `README.md` with the start command, the `mount` request with an example, the error codes, the `/dev/fuse` requirement, the dead mounts after a stop, the IDE notice, and the advice to bind to `127.0.0.1`. Verify that the documented `curl` example works against the daemon in the test container.

## 6. Deterministic tests

- [x] 6.1 Remove time measurements from the tests. The source-change tests wait past the 1-second cache timeout once and then check the view. The concurrent test waits for one `stat` and one create between config changes. Verify that the whole suite passes three times in a row.

## 7. Overlay directories on demand

- [x] 7.1 Show an overlay directory only when `overlay.src/<p>/<n>` exists, and send creates of overlay names to `overlay.src/<p>`. Verify with tests for "New project has no overlay directory", "Source .idea without an overlay directory", "IDE creates .idea", and a create of `.idea` in a read-only project.
