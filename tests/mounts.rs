mod common;

use common::{Daemon, Tree};
use serde_json::{Value, json};

fn mounts(daemon: &Daemon) -> Value {
    let reply = daemon.call("mounts", json!(null));
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

#[test]
fn no_views() {
    let daemon = Daemon::start();
    assert_eq!(mounts(&daemon), json!([]));
    for params in [json!({}), json!([])] {
        assert_eq!(daemon.call("mounts", params)["result"], json!([]));
    }
    let (_, body) = daemon.post("/api", r#"{"jsonrpc": "2.0", "method": "mounts", "id": 1}"#);
    let reply: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reply["result"], json!([]));
}

#[test]
fn unexpected_params() {
    let daemon = Daemon::start();
    let reply = daemon.call("mounts", json!({"dst": "/d"}));
    assert_eq!(reply["error"]["code"], -32602);
}

#[test]
fn mounted_host() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(
        mounts(&daemon),
        json!([{
            "src": tree.path("h1"),
            "overlay": {"src": tree.path("overlay"), "directories": [".idea"]},
            "dst": tree.path("d"),
            "squash": {"uid": 1000, "gid": 1000},
        }])
    );
}

#[test]
fn change_of_host() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h2", "d"));
    let result = mounts(&daemon);
    assert_eq!(result.as_array().unwrap().len(), 1);
    assert_eq!(result[0]["src"], json!(tree.path("h2")));
}

#[test]
fn two_views() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h2", "d2"));
    daemon.mount_ok(tree.params("h1", "d"));
    let result = mounts(&daemon);
    assert_eq!(result.as_array().unwrap().len(), 2);
    assert_eq!(result[0]["dst"], json!(tree.path("d")));
    assert_eq!(result[1]["dst"], json!(tree.path("d2")));
}

#[test]
fn restored_view_after_a_restart() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.restart();
    assert_eq!(
        mounts(&daemon),
        json!([{
            "src": null,
            "overlay": {"src": tree.path("overlay"), "directories": [".idea"]},
            "dst": tree.path("d"),
            "squash": {"uid": 1000, "gid": 1000},
        }])
    );
}

#[test]
fn restored_view_after_umount() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let reply = daemon.call("umount", json!({"dst": tree.path("d")}));
    assert!(reply.get("error").is_none(), "{reply}");
    let result = mounts(&daemon);
    assert_eq!(result.as_array().unwrap().len(), 1);
    assert_eq!(result[0]["src"], Value::Null);
    assert_eq!(result[0]["overlay"]["src"], json!(tree.path("overlay")));
}

#[test]
fn unmounted_view() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["overlay"] = json!(null);
    daemon.mount_ok(params);
    let reply = daemon.call("umount", json!({"dst": tree.path("d")}));
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(mounts(&daemon), json!([]));
}
