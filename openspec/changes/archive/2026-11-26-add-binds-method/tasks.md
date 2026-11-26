# Tasks

## 1. Method

- [x] 1.1 In `src/mounts.rs`, add `Registry::binds`, which returns the stored pairs as `StoredBind` items in `src` order. In `src/api.rs`, move the params check of `mounts` into one function that names the method in its message, and add the method `binds`. Verify: `tests/binds.rs` has a test for each scenario of "Binds method", and the `mounts` test "Unexpected params" still passes.

## 2. Docs

- [x] 2.1 Update `README.md`: a `binds` section with an example call and result after the `unbind` section, the note that a pair whose bind failed at start is listed, and `binds` in the `-32601` row. Verify: every JSON literal in the README parses.

## 3. Integration

- [x] 3.1 Run `docker compose --profile check run --rm check` and `docker compose --profile test run --rm test`. Verify: both exit 0.
