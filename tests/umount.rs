mod common;

use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;

use common::{Daemon, Tree, is_mount_point, names};
use serde_json::{Value, json};

fn without_overlay(tree: &Tree, host: &str, dst: &str) -> Value {
    let mut params = tree.params(host, dst);
    params["overlay"] = json!(null);
    params
}

fn umount(daemon: &Daemon, dst: impl serde::Serialize) -> Value {
    daemon.call("umount", json!({"dst": dst}))
}

#[test]
fn dst_not_served() {
    let tree = Tree::new();
    let daemon = Daemon::start();
    assert_eq!(umount(&daemon, tree.path("d"))["error"]["code"], -32000);
    assert_eq!(
        umount(&daemon, tree.path("missing"))["error"]["code"],
        -32000
    );
}

#[test]
fn relative_dst() {
    let daemon = Daemon::start();
    assert_eq!(umount(&daemon, "d")["error"]["code"], -32602);
}

#[test]
fn state_file_stays() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let before = fs::read(daemon.data_dir().join("state.json")).unwrap();
    assert_eq!(umount(&daemon, tree.path("d"))["result"], Value::Null);
    assert_eq!(
        fs::read(daemon.data_dir().join("state.json")).unwrap(),
        before
    );
}

#[test]
fn pet_view() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.pet_params("h1", "d"));
    let before = fs::read(daemon.data_dir().join("state.json")).unwrap();
    let reply = umount(&daemon, tree.path("d"));
    assert_eq!(reply["error"]["code"], -32000, "{reply}");
    assert!(is_mount_point(&tree.path("d")));
    assert_eq!(names(&tree.path("d")), ["proj1", "proj3"]);
    assert_eq!(
        fs::read(daemon.data_dir().join("state.json")).unwrap(),
        before
    );
}

#[test]
fn back_to_the_restored_view() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let inode = fs::metadata(tree.path("d/proj1/.idea")).unwrap().ino();
    let reply = umount(&daemon, tree.path("d"));
    assert_eq!(reply["result"], Value::Null, "{reply}");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(is_mount_point(&tree.path("d")));
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
    assert_eq!(names(&tree.path("d/proj1")), [".idea"]);
    assert_eq!(
        fs::metadata(tree.path("d/proj1/.idea")).unwrap().ino(),
        inode
    );
}

#[test]
fn mount_after_umount() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(umount(&daemon, tree.path("d"))["result"], Value::Null);
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(names(&tree.path("d/proj1")), [".idea", "main.py"]);
}

#[test]
fn plain_directory_again() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(without_overlay(&tree, "h1", "d"));
    assert!(is_mount_point(&tree.path("d")));
    let reply = umount(&daemon, tree.path("d"));
    assert_eq!(reply["result"], Value::Null, "{reply}");
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(!is_mount_point(&tree.path("d")));
    assert!(names(&tree.path("d")).is_empty());
}

#[test]
fn open_file_during_umount() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(without_overlay(&tree, "h1", "d"));
    let mut open = fs::File::open(tree.path("d/proj1/main.py")).unwrap();
    let reply = umount(&daemon, tree.path("d"));
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(!is_mount_point(&tree.path("d")));
    let mut content = String::new();
    open.read_to_string(&mut content).unwrap();
    assert_eq!(content, "h1 proj1\n");
}

#[test]
fn mount_again() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(without_overlay(&tree, "h1", "d"));
    assert_eq!(umount(&daemon, tree.path("d"))["result"], Value::Null);
    daemon.mount_ok(without_overlay(&tree, "h2", "d"));
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h2 proj1\n"
    );
}
