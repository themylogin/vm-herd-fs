# Tasks

## 1. Resolution and create

- [x] 1.1 In `src/mounts.rs`, add `dst_dir(path)` that resolves an existing `dst` as today and a missing one through its canonical parent plus file name, and use it for `dst` in `mount`. Verify: `tests/api.rs` has a test for `dst` `/missing/d` returning `-32602` with `/missing` still absent, a test for a regular file as `dst` returning `-32602`, and the `dst_inside_src` test passes with `h1/view` not pre-created and still absent after the call.
- [x] 1.2 In the not-served branch of `mount`, create a missing `dst` with mode `0755` and chown it to `squash` when set, mapping errors to `-32000`. Verify: `tests/api.rs` has a test that mounts `h1` without overlay at a missing `d3` with squash 1000, sees the listing, calls `umount`, and asserts owner 1000:1000 and mode `0o755`; a test without squash asserting owner 0:0; and a test that an existing `d` owned by 1001:1001 keeps that owner after mount and umount.

## 2. Docs

- [x] 2.1 Update `README.md`: the `dst` row says the daemon creates a missing `dst` with mode `0755` and, with `squash`, owner `squash.uid:squash.gid`; the paths paragraph says `dst` may be missing when its parent exists. Verify: the README has no sentence left that requires `dst` to exist.

## 3. Integration

- [x] 3.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
