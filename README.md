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
| `src` | The host directory. `dst` is a passthrough of it. Each directory in it is a project. |
| `overlay` | `null`, or the store for IDE directories. |
| `overlay.src` | Holds `<project>/<name>` for each project. Each directory in it is a project too. A project that `src` does not have shows as a read-only directory. |
| `overlay.directories` | Names such as `.idea`. `dst/<project>/<name>` shows `overlay.src/<project>/<name>` when it exists. A create of `dst/<project>/<name>` creates it there. The same names in `src` stay hidden and untouched. |
| `dst` | The mount point. |
| `squash` | `null`, or `{"uid", "gid"}`. With `squash`, every file shows as `uid:gid`, new files get owner 0:0 on disk, and chown changes nothing on disk. Without it, files show their real owners and new files belong to the caller. |

All paths must be absolute paths of existing directories. `dst` must not overlap `src`,
`overlay.src`, or another served `dst`.

A `mount` call for a `dst` that the daemon already serves changes the view in place. Use this
to change the host. A `mount` call for a `dst` with another mount at or below it fails.

| Error code | Cause |
|---|---|
| `-32700` | The body is not JSON. |
| `-32600` | The body is not a JSON-RPC 2.0 request. |
| `-32601` | The method is not `mount` or `umount`. |
| `-32602` | A param is wrong. The message names it. |
| `-32000` | The call failed, for example because of a foreign mount at `dst`, or `umount` for a `dst` that is not served. |

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

`umount` does not change `state.json`. For a `dst` that the daemon does not serve, it fails
with `-32000`.

## Changes made outside the view

Changes in `src`, for example from NFS clients, show in `dst` within 1 second. The view sends
no inotify events. JetBrains IDEs show "External file changes sync may be slow" for the view.
They refresh when their window gets focus, or with "File | Reload All from Disk".

## Restart

After each successful `mount`, the daemon writes `<data-dir>/state.json`:

```json
{
  "version": 1,
  "mounts": [
    {
      "dst": "/home/user/Dev/truenas",
      "overlay": {"src": "/home/user/Dev/.vm-herd/truenas/workdir/overlay", "directories": [".idea"]},
      "squash": {"uid": 1000, "gid": 1000}
    }
  ]
}
```

The file holds each `dst` whose last `mount` had an `overlay`. A `mount` without `overlay`
removes the `dst` from the file.

When the daemon stops or crashes, each `dst` stays as a dead mount ("Transport endpoint is not
connected"). At the next start, the daemon detaches its earlier mount at each stored `dst`
and mounts a view without `src`: every project of `overlay.src` shows with only its overlay
directories, and the root is read-only. The next `mount` call for that `dst` replaces the view
in place. Only one daemon can run on a host.

The daemon skips a stored `dst` that is missing or holds another mount, prints the reason,
and keeps the item in the file. It does not start when `state.json` has an unknown `version`.

## Checks

```sh
docker compose --profile check run --rm check
docker compose --profile test run --rm test
```
