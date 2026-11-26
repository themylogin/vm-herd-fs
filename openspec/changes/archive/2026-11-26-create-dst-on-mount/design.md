# Design

## Context

See proposal.md for the motivation. `Registry::mount` in `mounts.rs` resolves every path with
`existing_dir`, which canonicalizes and fails with `-32602` when the path is missing. The
overlap checks (`check_apart`) and the served-`dst` check run on the resolved `dst`. The daemon
sets `umask(0)` at start, so a `mkdir` gets the mode it asks for. `restore_item` has its own
`is_dir` check on `dst` and is out of scope.

## Goals / Non-Goals

**Goals:**
- Keep "no change before the checks pass": the directory appears only after every `-32602`
  case is ruled out.
- One resolution function for `dst`, so the overlap checks see the same path whether `dst`
  exists or not.

**Non-Goals:**
- Parent directories. A missing parent is a wrong param.
- A change of owner or mode on an existing `dst`.
- A create at restore. The stored `dst` is on a persistent disk and exists after a reboot.
- A cleanup of the new directory when `attach` fails afterwards. A later `mount` reuses it.

## Decisions

### `dst` resolves through its parent when missing

A new `dst_dir(path) -> Result<PathBuf, MountError>` replaces `existing_dir("dst", ..)` in
`mount`. It requires an absolute path. When the path exists, it canonicalizes it and requires a
directory, as today. Otherwise it canonicalizes `path.parent()` (a failure is `-32602` with
"dst <path>: <err>"), takes `path.file_name()` (`None`, as for `/a/..`, is `-32602`), and
returns `parent.join(name)`. The overlap and served-`dst` checks run on that path unchanged.

Alternative: skip the checks for a missing `dst` and create first. Rejected: `dst` `/w/h1/view`
inside `src` would get created before the check rejects it.

### Create right before `attach`

In the not-served branch of `mount`, after the foreign-mount check:

```rust
if !dst.exists() {
    rustix::fs::mkdir(&dst, Mode::from_raw_mode(0o755))?;
    if let Some(squash) = params.squash {
        rustix::fs::chown(&dst, Some(Uid::from_raw(squash.uid)), Some(Gid::from_raw(squash.gid)))?;
    }
}
```

Both errors map to `MountError::Failed("create dst <path>: <err>")`, which is `-32000`. A
served `dst` always exists, so the in-place branch needs nothing.

### Owner on the directory, not through the view

The `chown` targets the real directory under the mount. While the view is mounted, `stat` on
`dst` shows the view root with `squash` applied either way. The on-disk owner matters when
nothing is mounted: after `umount` of a cattle view without overlay, or before a restore. The
tests observe it that way.

## Risks / Trade-offs

- [A typo in `dst` now creates a directory instead of failing] → The parent must still exist,
  which catches a wrong prefix. The README states that `mount` creates `dst`.
- [`dst` created, then `attach` fails] → The directory stays, owned as requested. The next
  `mount` finds an existing directory and mounts into it.
