# Design

## Context

See proposal.md for the motivation. Today `Config.overlay: Option<Overlay { src, directories }>`
in `view.rs` drives every root-level decision: `is_project`, `root_entries`, `check_fixed`,
`root_stat`, and `statfs`. Project children go through `Node::Project`, and everything else is a
`Node::Real` keyed by `(dev, ino)` of an `O_PATH` descriptor. `replace()` invalidates the root,
the project ids, and every name recorded under a non-`Real` parent. Nothing is deployed, so
JSON and state shapes can change without a migration.

The delta spec extends the example paths of the main spec. `/w/shared` holds the file
`notes.md` and the directory `tools` with `run.sh`. "Mount h1" uses `overlay`
`{"individual": {"src": "/w/overlay", "directories": [".idea"]}, "shared": {"src": "/w/shared"}}`.
The main spec's Purpose section must say this after archive; the task list carries that edit.

## Goals / Non-Goals

**Goals:**
- Shared entries reuse the `Node::Real` path. No new node kind.
- A change of host keeps shared entry inodes without new invalidation code.
- One rule for root collisions: shared wins.

**Non-Goals:**
- Shared entries inside projects (`dst/<p>/<e>`).
- A read of the old `state.json` or old `overlay` shape.
- A check that `shared.src`, `individual.src`, and `src` are apart from each other. Today
  `src` and `overlay.src` are not checked either.

## Decisions

### JSON stays nested, `Config` goes flat

`OverlayParams` becomes `{ individual: Option<Individual>, shared: Option<Shared> }` in
`api.rs`, and `StoredMount.overlay` keeps that type. `Config` in `view.rs` holds
`individual: Option<Individual>` and `shared: Option<Shared>` directly. Every
`config.overlay.as_ref().and_then(...)` chain reads one field instead.

Alternative: nest `Config.overlay` too. Rejected: it adds a level to every read in `view.rs`
and buys nothing, because the JSON shape is only a wire format.

### `overlay` and its members default to `null`

`overlay` is absent, `null`, or `{"individual": ..., "shared": ...}`. Each member is absent,
`null`, or an object. An absent key means `null`. Serde gives this for free: a missing
`Option<T>` field deserializes to `None` without `#[serde(default)]`. `deny_unknown_fields` on
all three structs rejects the old shape with `-32602`.

`mounts` and `state.json` always write both members, `null` included, so readers see one shape.

Alternative: drop the outer `null` and require `{"individual": null, "shared": null}` for no
overlay. Rejected: more verbose at the call site, and the state file rule "an item exists when
the last `mount` had an `overlay`" keeps its shape.

### Root lookup order: shared, then project, then src entry

`lookup(ROOT_ID, name)` checks `shared.src/<name>` first with `open_path`. A hit becomes a
`Real` node. Otherwise `is_project(name)` and the `src` real entry run as today. `is_project`
returns `false` when `shared.src` has the name, so `created_entry` and `root_entries` agree
with `lookup`.

`root_entries` lists `shared.src` first and seeds the `names` set. `src` entries and
`individual.src` directories with a name in the set are skipped. This gives the shadow rule in
the listing with the same code that already hides `src` names behind individual projects.

### Fixed shared entries through `check_fixed`

`check_fixed(Root, name)` becomes `is_overlay_only_project(name) || shared_has(name)`. `remove`
and `rename` already call it for the source and the target name, so both "remove `dst/<e>`" and
"rename onto `dst/<e>`" return `EPERM` with no new call sites. Nested entries have a `Real`
parent, and `check_fixed(Real, _)` stays `false`.

### Root stat and statfs fall back to `shared.src`

`root_stat` and `statfs` use `src`, else `individual.src`, else `shared.src`. A restored view
with `individual: null` needs this to stat at all.

### State file keeps version 1

The shape under `overlay` changes; `version` stays `1`. `state::load` fails on an old file
because of `deny_unknown_fields`, and `main` prints the serde error and exits non-zero. This is
the "not valid" branch of the existing state file version requirement.

Alternative: bump to version 2. Rejected: a version bump signals a migration path that does not
exist and that nobody needs.

## Risks / Trade-offs

- [A shared directory named like a project hides the project and its `.idea`] → The spec states
  the rule in "Shared entries". The README param table says it in one sentence.
- [Each root lookup gains one `openat` on `shared.src`] → Negligible next to the existing
  `statat` on `src`. The kernel caches the entry for `TTL`.
- [Old `state.json` stops the daemon] → Nothing is deployed. The error message names the file
  and the field, so the fix is to delete the file.
