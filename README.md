# vm-herd-fs

A daemon that shows the projects of one host at a stable path through FUSE. The IDE opens the
stable path. A JSON-RPC call changes the host without an unmount, so projects and their
`.idea` directories never disappear.

## Requirements

- Linux with `/dev/fuse`.
- The daemon runs as root. It mounts with `mount(2)` directly and does not need `fusermount3`.

## Build

```sh
docker compose --profile check run --rm -v "$PWD/dist:/dist" check \
    sh -c 'cargo build --release --target x86_64-unknown-linux-musl \
        && cp target/x86_64-unknown-linux-musl/release/vm-herd-fs /dist/'
```

The binary is static, so it runs on old distributions too.

## Start

```sh
sudo ./dist/vm-herd-fs --listen 127.0.0.1:8742 --data-dir /var/lib/vm-herd-fs
```

The daemon creates the data directory when it is missing. It prints `listening on <address>`. The API has no authentication. Any process that can
reach the address can mount any directory as root. Bind to `127.0.0.1`.

## API

The daemon accepts JSON-RPC 2.0 requests at `POST /api`. The method `mount` shows `src` at
`dst`:

```sh
curl -s http://127.0.0.1:8742/api -d '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "mount",
  "params": {
    "type": "cattle",
    "src": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130",
    "overlay": {"src": "/home/user/Dev/.vm-herd/truenas/workdir/overlay", "directories": [".idea"]},
    "dst": "/home/user/Dev/truenas",
    "squash": {"uid": 1000, "gid": 1000}
  }
}'
```

On success, the result is `null`.

| Param | Meaning |
|---|---|
| `type` | `cattle` or `pet`. A cattle view is described below. A pet view shows `src` at `dst` with `squash` and nothing else: `overlay` must be absent or `null`. A pet comes back after a daemon start, and `umount` refuses it. |
| `src` | The host directory. `dst` is a passthrough of it. Each directory in it is a project. |
| `overlay` | `null`, or the store for IDE directories: `{"src", "directories"}`. Absent means `null`. |
| `overlay.src` | Holds `<project>/<name>` for each project. Each directory in it is a project too. A project that `src` does not have shows as a read-only directory. |
| `overlay.directories` | Names such as `.idea`. `dst/<project>/<name>` shows `overlay.src/<project>/<name>` when it exists. A create of `dst/<project>/<name>` creates it there. The same names in `src` stay hidden and untouched. |
| `dst` | The mount point. When it does not exist, the daemon creates it with mode `0755`. With `squash`, the new directory gets owner `squash.uid:squash.gid`. An existing `dst` stays as it is. |
| `squash` | `null`, or `{"uid", "gid"}`. With `squash`, every file shows as `uid:gid`, new files get owner 0:0 on disk, and chown changes nothing on disk. Without it, files show their real owners and new files belong to the caller. |

All paths must be absolute. `src` and `overlay.src` must be existing directories. `dst` may be
missing when its parent exists. `dst` must not overlap `src`, `overlay.src`, or another served
`dst`.

A pet mount:

```sh
curl -s http://127.0.0.1:8742/api -d '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "mount",
  "params": {
    "type": "pet",
    "src": "/home/user/Dev/.vm-herd/pets/workstation",
    "dst": "/home/user/Dev/workstation",
    "squash": {"uid": 1000, "gid": 1000}
  }
}'
```

A `mount` call for a `dst` that the daemon already serves changes the view in place. Use this
to change the host. A `mount` call for a `dst` with another mount at or below it fails.

| Error code | Cause |
|---|---|
| `-32700` | The body is not JSON. |
| `-32600` | The body is not a JSON-RPC 2.0 request. |
| `-32601` | The method is not `mount`, `umount`, `mounts`, `bind`, `unbind`, or `binds`. |
| `-32602` | A param is wrong. The message names it. |
| `-32000` | The call failed, for example because of a foreign mount at `dst`, `umount` for a `dst` that is not served or holds a pet view, or a `bind` check. |

### umount

The method `umount` puts a served `dst` into the state that a reboot and a daemon start produce:

```sh
curl -s http://127.0.0.1:8742/api -d '{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "umount",
  "params": {"dst": "/home/user/Dev/truenas"}
}'
```

- When `state.json` holds `dst`, the view changes in place to the restored view: every project
  of `overlay.src` with only its overlay directories. The mount stays.
- Otherwise, the daemon unmounts `dst`. Open files do not block the call.

`umount` does not change `state.json`. For a `dst` that the daemon does not serve, or that holds
a pet view, it fails with `-32000`. To free a pet `dst`, mount a cattle view without `overlay`
there, then call `umount`.

### mounts

The method `mounts` takes no params and lists the views that the daemon serves, sorted by
`dst`:

```sh
curl -s http://127.0.0.1:8742/api -d '{"jsonrpc": "2.0", "id": 3, "method": "mounts"}'
```

```json
{"id": 3, "jsonrpc": "2.0", "result": [
  {
    "type": "cattle",
    "src": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130",
    "overlay": {"src": "/home/user/Dev/.vm-herd/truenas/workdir/overlay", "directories": [".idea"]},
    "dst": "/home/user/Dev/truenas",
    "squash": {"uid": 1000, "gid": 1000}
  }
]}
```

Each item has the form of the `mount` params, with the config that the view shows now. `src`
is `null` for a restored cattle view, after a daemon start or after `umount`. A pet item shows
its `src` and `overlay` `null`. Bind mounts do not show. Params other than none, `null`, `{}`,
or `[]` fail with `-32602`.

### bind

The method `bind` puts the entries of a shared store `src` into a host directory `dst`, so the
NFS clients of that directory see them:

```sh
curl -s http://127.0.0.1:8742/api -d '{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "bind",
  "params": {
    "src": "/home/user/Dev/.vm-herd/truenas/workdir/shared",
    "dst": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130"
  }
}'
```

The daemon bind-mounts each file and directory of `src` at `dst/<entry>`. A bound directory
stays shared both ways: creates and removes inside it show in `dst` and in the store at once.
Views do not change bind mounts, so each host directory keeps its entries while a view shows
another host. A view shows the bound entries like any other entry of its own `src`.

- Both params must be absolute paths of existing directories.
- When `dst/<entry>` is missing, the daemon creates an empty file (`0644`) or directory
  (`0755`), owned by 0:0, for the mount.
- `bind` fails with `-32000` and changes nothing when `dst/<entry>` is a symlink, when it is a
  file and the store entry is a directory or the reverse, or when a store entry is neither a
  file nor a directory.
- A later `bind` for the same `dst` adds the entries that the store got, removes the bind
  mounts of entries that are gone, and switches to another store when `src` changes. It
  never puts a second bind mount on a path.
- An entry of the same name in `dst` stays hidden and unchanged under the bind mount.
- An NFS client cannot remove or rename onto a top-level bound file. The call fails with
  `EBUSY`. Editors that save through a rename fail on such a file. Files inside a bound
  directory have no such limit.

### unbind

The method `unbind` removes the bind mounts of a host directory:

```sh
curl -s http://127.0.0.1:8742/api -d '{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "unbind",
  "params": {"dst": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130"}
}'
```

After each removal, the daemon also removes `dst/<entry>` when it is an empty file or an empty
directory. A real empty entry of that name goes away too. Open files do not block the call.
For a `dst` without bind mounts and without an item in `state.json`, `unbind` fails with
`-32000`.

Call `unbind` before you delete a host directory. `rm -rf` goes through a bound directory and
deletes the files in the store. `--one-file-system` does not stop it, because the bind mount is
on the same file system.

### binds

The method `binds` takes no params and lists the stored binds, sorted by `dst`:

```sh
curl -s http://127.0.0.1:8742/api -d '{"jsonrpc": "2.0", "id": 6, "method": "binds"}'
```

```json
{"id": 6, "jsonrpc": "2.0", "result": [
  {
    "src": "/home/user/Dev/.vm-herd/truenas/workdir/shared",
    "dst": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130"
  }
]}
```

The result holds the same pairs as `binds` in `state.json`, with resolved paths. A pair whose
bind failed at start is listed too, so `unbind` can remove it. Params other than none, `null`,
`{}`, or `[]` fail with `-32602`.

### NFS

The NFS export of each `src` needs the option `crossmnt`, for example
`/home/user/Dev/.vm-herd <client>(rw,no_subtree_check,crossmnt)`. Without it, clients see the
empty mount points. The daemon must run in the host mount namespace, for example not under a
systemd unit with `PrivateMounts=`, or `nfsd` does not see the bind mounts.

After an NFS client crossed into a bind mount, `nfsd` holds it, and a plain `umount` fails with
"target is busy". The daemon always detaches with `umount -l` semantics, so it never blocks.
A client that still has its submount sees the old entry or "Stale file handle" until it mounts
`src` again.

## Changes made outside the view

Changes in `src`, for example from NFS clients, show in `dst` within 1 second. The view sends
no inotify events. JetBrains IDEs show "External file changes sync may be slow" for the view.
They refresh when their window gets focus, or with "File | Reload All from Disk".

## Restart

After each successful `mount`, `bind`, and `unbind`, the daemon writes
`<data-dir>/state.json`:

```json
{
  "version": 1,
  "mounts": [
    {
      "type": "cattle",
      "dst": "/home/user/Dev/truenas",
      "overlay": {"src": "/home/user/Dev/.vm-herd/truenas/workdir/overlay", "directories": [".idea"]},
      "squash": {"uid": 1000, "gid": 1000}
    },
    {
      "type": "pet",
      "dst": "/home/user/Dev/workstation",
      "src": "/home/user/Dev/.vm-herd/pets/workstation",
      "squash": {"uid": 1000, "gid": 1000}
    }
  ],
  "binds": [
    {
      "src": "/home/user/Dev/.vm-herd/truenas/workdir/shared",
      "dst": "/home/user/Dev/.vm-herd/truenas/workdir/192.168.2.130"
    }
  ]
}
```

`mounts` holds each cattle `dst` whose last `mount` had an `overlay`, and each pet `dst`. A
cattle `mount` without `overlay` removes the `dst` from the file. `binds` holds each `dst`
whose last call was a successful `bind`.

A `state.json` from a version before `bind`, or with `binds` items that hold `shared`, has
another shape, and the daemon does not start with it. Remove the file once before you start
the new version.

When the daemon stops or crashes, each `dst` stays as a dead mount ("Transport endpoint is not
connected"), and the bind mounts stay and keep working. At the next start, the daemon first
binds each stored `binds` item again. A bind mount that is still there stays, and the daemon
never adds a second one. Then the daemon detaches its earlier mount at each stored `dst` and
mounts a view without `src`: every project of `overlay.src` shows with only its overlay
directories, and the root is read-only. For a pet item, it mounts the stored `src` at `dst`
again. The next `mount` call for that `dst` replaces the view in place. Only one daemon can run
on a host.

The daemon skips a stored `dst` that is missing or holds another mount, a pet whose `src` is
missing, and a `binds` item that fails the checks of `bind`. It prints the reason and keeps
the item in the file. It does not start when `state.json` has an unknown `version`.

## Checks

```sh
docker compose --profile check run --rm check
docker compose --profile test run --rm test
```
