mod common;

use std::process::Command;

use common::{Daemon, Tree, is_mount_point, names};
use serde_json::json;

#[test]
fn successful_mount() {
    let tree = Tree::new();
    std::fs::create_dir(tree.path("empty")).unwrap();
    let mut daemon = Daemon::start();
    daemon.mount_ok(
        json!({"type": "cattle", "src": tree.path("empty"), "overlay": null, "dst": tree.path("d"), "squash": null}),
    );
    assert!(is_mount_point(&tree.path("d")));
    assert!(names(&tree.path("d")).is_empty());
}

#[test]
fn unknown_method() {
    let daemon = Daemon::start();
    let reply = daemon.call("unmount", json!({}));
    assert_eq!(reply["error"]["code"], -32601);
    assert_eq!(reply["id"], 1);
}

#[test]
fn parse_error() {
    let daemon = Daemon::start();
    let (status, body) = daemon.post("/api", "{not json");
    assert_eq!(status, 200);
    let reply: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reply["error"]["code"], -32700);
}

#[test]
fn other_path_and_method() {
    let daemon = Daemon::start();
    assert_eq!(daemon.post("/other", "{}").0, 404);
    assert_eq!(daemon.get("/api"), 405);
}

#[test]
fn invalid_request() {
    let daemon = Daemon::start();
    let (_, body) = daemon.post("/api", r#"{"method": "mount", "id": 7}"#);
    let reply: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(reply["error"]["code"], -32600);
    assert_eq!(reply["id"], 7);
}

#[test]
fn relative_path() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["src"] = json!("w/h1");
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    assert!(!is_mount_point(&tree.path("d")));
}

#[test]
fn dst_inside_src() {
    let tree = Tree::new();
    std::fs::create_dir(tree.path("h1/view")).unwrap();
    let mut daemon = Daemon::start();
    let reply = daemon.mount(tree.params("h1", "h1/view"));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let reply = daemon.mount(tree.params("h1", "overlay/proj1"));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}

#[test]
fn dst_inside_served_dst() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let reply = daemon.mount(tree.params("h2", "d/proj2/.idea"));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}

#[test]
fn name_with_a_slash() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["overlay"]["individual"]["directories"] = json!(["a/b"]);
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("overlay.individual.directories"),
        "{message}"
    );
}

#[test]
fn overlay_without_members() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["overlay"] = json!({"src": tree.path("overlay"), "directories": [".idea"]});
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    assert!(!is_mount_point(&tree.path("d")));
}

#[test]
fn relative_shared_src() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["overlay"]["shared"] = json!({"src": "w/shared"});
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains("overlay.shared.src"), "{message}");
}

#[test]
fn missing_type() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params.as_object_mut().unwrap().remove("type");
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains("type"), "{message}");
    assert!(!is_mount_point(&tree.path("d")));
}

#[test]
fn unknown_type() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["type"] = json!("sheep");
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    assert!(!is_mount_point(&tree.path("d")));
}

#[test]
fn pet_with_an_overlay() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.pet_params("h1", "d");
    params["overlay"] = json!({"shared": {"src": tree.path("shared")}});
    let reply = daemon.mount(params);
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains("overlay"), "{message}");
    assert!(!is_mount_point(&tree.path("d")));
}

#[test]
fn overlay_omitted() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(
        json!({"type": "cattle", "src": tree.path("h1"), "dst": tree.path("d"), "squash": null}),
    );
    assert_eq!(names(&tree.path("d")), ["proj1", "proj3"]);
}

#[test]
fn old_bindfs_mount() {
    let tree = Tree::new();
    let dst = tree.path("d");
    let status = Command::new("mount")
        .args(["-t", "tmpfs", "tmpfs"])
        .arg(&dst)
        .status()
        .unwrap();
    assert!(status.success());
    let mut daemon = Daemon::start();
    let reply = daemon.mount(tree.params("h1", "d"));
    assert_eq!(reply["error"]["code"], -32000, "{reply}");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains(dst.to_str().unwrap()), "{message}");
    assert!(message.contains("tmpfs"), "{message}");
    assert!(is_mount_point(&dst));
    assert!(Command::new("umount").arg(&dst).status().unwrap().success());
}
