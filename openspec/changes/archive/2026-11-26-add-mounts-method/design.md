# Design

## Context

See proposal.md for the motivation and specs/project-view/spec.md for the new requirements.

Current code:

- `Registry.mounts` in `src/mounts.rs` is a `HashMap<PathBuf, Mounted>` keyed by the resolved `dst`. `Mounted` holds the `fuser` session, its notifier, and the `View`.
- The `View` holds a `Config` with open `O_PATH` descriptors for `src` and `overlay.src`. It keeps no paths, so the registry cannot report what a view shows.
- Four code paths set the config of a served `dst`:
  - `attach`, called by `mount` for a new `dst` and by the restore at start;
  - `View::replace` in `mount` for a served `dst`;
  - `View::replace` in `umount` for a `dst` with a stored item;
  - removal from the registry in `umount` for a `dst` without a stored item.
- `api.rs` reads `params` as `Value::Null` when the request has none.

## Goals / Non-Goals

**Goals:**

- `mounts` reports exactly the config each view shows, including restored views.

**Non-Goals:**

- Mounts that this daemon does not serve, such as foreign mounts or skipped state items.
- Live checks of the views, such as whether `src` still exists.

## Decisions

### Record the shown config next to each session

`Mounted` gains a field `shown: Shown` with `src: Option<PathBuf>`, `overlay: Option<OverlayParams>`, and `squash: Option<Squash>`. Each path that sets a config also sets `shown`:

| Path | `shown` |
|---|---|
| `mount`, new or served `dst` | `src`, `overlay`, `squash` of the call |
| restore at start | `src: None`, the stored `overlay` and `squash` |
| `umount` with a stored item | `src: None`, the stored `overlay` and `squash` |
| `umount` without a stored item | the entry is gone |

`attach` takes `shown` as a parameter, so a new entry cannot exist without it. `src` and `overlay` keep the paths as the caller sent them, the same as `state.json`. Only `dst` is the resolved path, because it is the registry key.

Alternative: keep paths in the `View` config. The view needs only descriptors, and the shown config is registry data.

### Sorted result

`Registry.mounts` becomes a `BTreeMap`, so iteration gives the items sorted by `dst` without a separate sort. No code depends on the `HashMap` order.

### Result type and params

`api.rs` defines `MountsItem { src, overlay, dst, squash }` with `Serialize`. The reply lists the keys in alphabetical order, because `json!` builds a sorted map; JSON clients do not depend on key order. `Registry::mounts` returns the items. The `mounts` handler accepts params that are absent, `null`, an empty object, or an empty array, and returns `-32602` with the message `mounts takes no params` otherwise.

## Risks / Trade-offs

- [A `src` reported by `mounts` was renamed after the call that mounted it] → `mounts` reports the config, not the current state of the paths. This matches `state.json`.
