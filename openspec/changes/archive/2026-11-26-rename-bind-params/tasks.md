# Tasks

## 1. Code

- [x] 1.1 Rename the params: `BindParams { src, dst }` and `UnbindParams { dst }` in `src/api.rs`; `StoredBind { src, dst }` in `src/state.rs`; the stored pairs keyed by `dst` in `src/mounts.rs`, with the error messages naming `src` and `dst`. Verify: `check` exits 0 and the `state.rs` unit test round-trips a `{"src", "dst"}` item.
- [x] 1.2 Update the tests: `Tree::bind_params` sends `{"src": <shared>, "dst": <host>}`, every `unbind` call sends `dst`, and every `binds` literal is `{"src", "dst"}`. Verify: `test` exits 0, and `grep -rn '"shared"' tests` finds only the pet overlay tests that expect `-32602`.

## 2. Docs

- [x] 2.1 Update `README.md` for the new names in the `bind`, `unbind`, `binds`, and `state.json` sections, and say that a `state.json` with old `binds` items must be removed once. Update the Purpose section of `openspec/specs/project-view/spec.md`: "bind h1" means a `bind` call with `src` `/w/shared` and `dst` `/w/h1`. Verify: every JSON literal in the README parses, and `openspec validate rename-bind-params --strict` passes.
