mod common;

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use common::{Daemon, Tree, is_mount_point, mount_count, names};
use serde_json::{Value, json};

fn bound() -> (Tree, Daemon) {
    let tree = Tree::new();
    let daemon = Daemon::start();
    daemon.bind_ok(tree.bind_params("h1"));
    (tree, daemon)
}

fn unbind(daemon: &Daemon, tree: &Tree) -> Value {
    daemon.call("unbind", json!({"dst": tree.path("h1")}))
}

fn failed_message(reply: &Value) -> String {
    assert_eq!(reply["error"]["code"], -32000, "{reply}");
    reply["error"]["message"].as_str().unwrap().to_owned()
}

fn state(daemon: &Daemon) -> Value {
    serde_json::from_str(&fs::read_to_string(daemon.data_dir().join("state.json")).unwrap())
        .unwrap()
}

fn detach(path: &std::path::Path) {
    let status = Command::new("umount").arg("-l").arg(path).status().unwrap();
    assert!(status.success());
}

#[test]
fn shared_file_in_src() {
    let (tree, _daemon) = bound();
    assert!(is_mount_point(&tree.path("h1/notes.md")));
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "shared notes\n"
    );
}

#[test]
fn create_inside_a_shared_directory_in_src() {
    let (tree, _daemon) = bound();
    fs::write(tree.path("h1/tools/new.sh"), "").unwrap();
    assert!(tree.path("shared/tools/new.sh").exists());
}

#[test]
fn remove_inside_a_shared_directory_in_the_store() {
    let (tree, _daemon) = bound();
    fs::remove_file(tree.path("shared/tools/run.sh")).unwrap();
    assert!(!tree.path("h1/tools/run.sh").exists());
}

#[test]
fn shared_entry_hides_a_src_entry() {
    let tree = Tree::new();
    fs::write(tree.path("h1/notes.md"), "host notes").unwrap();
    let daemon = Daemon::start();
    daemon.bind_ok(tree.bind_params("h1"));
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "shared notes\n"
    );
    assert_eq!(unbind(&daemon, &tree)["result"], Value::Null);
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "host notes"
    );
}

#[test]
fn two_hosts_share_one_store() {
    let (tree, daemon) = bound();
    daemon.bind_ok(tree.bind_params("h2"));
    assert!(is_mount_point(&tree.path("h1/notes.md")));
    assert!(is_mount_point(&tree.path("h2/notes.md")));
}

#[test]
fn same_store_keeps_one_bind_mount() {
    let (tree, daemon) = bound();
    daemon.bind_ok(tree.bind_params("h1"));
    assert_eq!(mount_count(&tree.path("h1/notes.md")), 1);
}

#[test]
fn store_entry_added_after_the_bind() {
    let (tree, daemon) = bound();
    fs::write(tree.path("shared/new.md"), "").unwrap();
    assert!(!tree.path("h1/new.md").exists());
    daemon.bind_ok(tree.bind_params("h1"));
    assert!(is_mount_point(&tree.path("h1/new.md")));
}

#[test]
fn store_entry_removed_after_the_bind() {
    let (tree, daemon) = bound();
    fs::remove_file(tree.path("shared/notes.md")).unwrap();
    daemon.bind_ok(tree.bind_params("h1"));
    assert!(!tree.path("h1/notes.md").exists());
}

#[test]
fn another_store() {
    let (tree, daemon) = bound();
    fs::create_dir(tree.path("other")).unwrap();
    fs::write(tree.path("other/notes.md"), "other notes").unwrap();
    daemon.bind_ok(json!({"src": tree.path("other"), "dst": tree.path("h1")}));
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "other notes"
    );
    assert!(!tree.path("h1/tools").exists());
    assert_eq!(mount_count(&tree.path("h1/notes.md")), 1);
}

#[test]
fn missing_mount_point() {
    let (tree, mut daemon) = bound();
    assert!(is_mount_point(&tree.path("h1/tools")));
    assert_eq!(names(&tree.path("h1/tools")), ["run.sh"]);
    daemon.kill();
    detach(&tree.path("h1/tools"));
    let meta = fs::symlink_metadata(tree.path("h1/tools")).unwrap();
    assert!(meta.is_dir());
    assert_eq!((meta.uid(), meta.gid()), (0, 0));
    assert_eq!(meta.permissions().mode() & 0o777, 0o755);
    assert!(names(&tree.path("h1/tools")).is_empty());
}

#[test]
fn symlink_at_the_mount_point() {
    let tree = Tree::new();
    symlink(tree.path("h2"), tree.path("h1/tools")).unwrap();
    let daemon = Daemon::start();
    let message = failed_message(&daemon.call("bind", tree.bind_params("h1")));
    assert!(
        message.contains(tree.path("h1/tools").to_str().unwrap()),
        "{message}"
    );
    assert!(!is_mount_point(&tree.path("h1/notes.md")));
    assert!(!is_mount_point(&tree.path("h2")));
}

#[test]
fn file_where_the_store_has_a_directory() {
    let tree = Tree::new();
    fs::write(tree.path("h1/tools"), "").unwrap();
    let daemon = Daemon::start();
    let message = failed_message(&daemon.call("bind", tree.bind_params("h1")));
    assert!(
        message.contains(tree.path("h1/tools").to_str().unwrap()),
        "{message}"
    );
    assert!(!is_mount_point(&tree.path("h1/notes.md")));
}

#[test]
fn store_entry_of_another_kind() {
    let tree = Tree::new();
    symlink(tree.path("shared/notes.md"), tree.path("shared/link")).unwrap();
    let daemon = Daemon::start();
    let message = failed_message(&daemon.call("bind", tree.bind_params("h1")));
    assert!(
        message.contains(tree.path("shared/link").to_str().unwrap()),
        "{message}"
    );
}

#[test]
fn failed_bind_again_keeps_the_bind_mounts() {
    let (tree, daemon) = bound();
    let before = state(&daemon);
    symlink(tree.path("shared/notes.md"), tree.path("shared/link")).unwrap();
    failed_message(&daemon.call("bind", tree.bind_params("h1")));
    assert!(is_mount_point(&tree.path("h1/notes.md")));
    assert_eq!(state(&daemon), before);
}

#[test]
fn unbind_removes_the_bind_mounts() {
    let (tree, daemon) = bound();
    assert_eq!(unbind(&daemon, &tree)["result"], Value::Null);
    assert!(!tree.path("h1/notes.md").exists());
    assert!(!tree.path("h1/tools").exists());
}

fn bound_with_exportfs() -> (Tree, Daemon) {
    let tree = Tree::new();
    fs::create_dir(tree.path("bin")).unwrap();
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> {}\n",
        tree.path("exportfs.log").display()
    );
    fs::write(tree.path("bin/exportfs"), script).unwrap();
    fs::set_permissions(tree.path("bin/exportfs"), fs::Permissions::from_mode(0o755)).unwrap();
    let daemon = Daemon::start_with_bin(&tree.path("bin"));
    daemon.bind_ok(tree.bind_params("h1"));
    (tree, daemon)
}

fn exportfs_calls(tree: &Tree) -> String {
    fs::read_to_string(tree.path("exportfs.log")).unwrap_or_default()
}

#[test]
fn bind_keeps_the_nfs_export_cache() {
    let (tree, _daemon) = bound_with_exportfs();
    assert_eq!(exportfs_calls(&tree), "");
}

#[test]
fn unbind_flushes_the_nfs_export_cache() {
    let (tree, daemon) = bound_with_exportfs();
    unbind(&daemon, &tree);
    assert_eq!(exportfs_calls(&tree), "-f\n");
}

#[test]
fn removed_store_entry_flushes_the_nfs_export_cache() {
    let (tree, daemon) = bound_with_exportfs();
    fs::remove_file(tree.path("shared/notes.md")).unwrap();
    daemon.bind_ok(tree.bind_params("h1"));
    assert_eq!(exportfs_calls(&tree), "-f\n");
}

#[test]
fn entry_with_content_stays() {
    let tree = Tree::new();
    fs::write(tree.path("h1/notes.md"), "host notes").unwrap();
    let daemon = Daemon::start();
    daemon.bind_ok(tree.bind_params("h1"));
    unbind(&daemon, &tree);
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "host notes"
    );
}

#[test]
fn open_file_during_unbind() {
    let (tree, daemon) = bound();
    let _open = fs::File::open(tree.path("h1/tools/run.sh")).unwrap();
    assert_eq!(unbind(&daemon, &tree)["result"], Value::Null);
    assert!(!is_mount_point(&tree.path("h1/tools")));
}

#[test]
fn src_without_bind_mounts() {
    let tree = Tree::new();
    let daemon = Daemon::start();
    failed_message(&unbind(&daemon, &tree));
}

#[test]
fn relative_src() {
    let daemon = Daemon::start();
    let reply = daemon.call("unbind", json!({"dst": "w/h1"}));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}

#[test]
fn stored_item_that_failed_at_start() {
    let (tree, mut daemon) = bound();
    daemon.kill();
    detach(&tree.path("h1/notes.md"));
    detach(&tree.path("h1/tools"));
    fs::remove_dir_all(tree.path("h1")).unwrap();
    daemon.restart();
    assert_eq!(unbind(&daemon, &tree)["result"], Value::Null);
    assert_eq!(state(&daemon)["binds"], json!([]));
}

#[test]
fn bind_is_stored() {
    let (tree, daemon) = bound();
    assert_eq!(
        state(&daemon)["binds"],
        json!([{"src": tree.path("shared"), "dst": tree.path("h1")}])
    );
}

#[test]
fn unbind_is_stored() {
    let (tree, daemon) = bound();
    unbind(&daemon, &tree);
    assert_eq!(state(&daemon)["binds"], json!([]));
}

#[test]
fn change_of_host_keeps_the_bind_mounts() {
    let (tree, mut daemon) = bound();
    daemon.bind_ok(tree.bind_params("h2"));
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h2", "d"));
    assert!(is_mount_point(&tree.path("h1/notes.md")));
    assert!(is_mount_point(&tree.path("h2/notes.md")));
}

#[test]
fn umount_keeps_the_bind_mounts() {
    let (tree, mut daemon) = bound();
    daemon.mount_ok(tree.params("h1", "d"));
    let reply = daemon.call("umount", json!({"dst": tree.path("d")}));
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(is_mount_point(&tree.path("h1/notes.md")));
}

#[test]
fn bind_under_a_served_view() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
    daemon.bind_ok(tree.bind_params("h1"));
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
}

#[test]
fn restart_after_a_crash() {
    let (tree, mut daemon) = bound();
    daemon.restart();
    assert_eq!(mount_count(&tree.path("h1/notes.md")), 1);
}

#[test]
fn restart_after_the_bind_mounts_went_away() {
    let (tree, mut daemon) = bound();
    daemon.kill();
    detach(&tree.path("h1/notes.md"));
    detach(&tree.path("h1/tools"));
    daemon.restart();
    assert!(is_mount_point(&tree.path("h1/notes.md")));
    assert_eq!(
        fs::read_to_string(tree.path("h1/notes.md")).unwrap(),
        "shared notes\n"
    );
}

#[test]
fn broken_mount_point_at_start() {
    let tree = Tree::new();
    fs::write(tree.path("h1/tools"), "").unwrap();
    let data_dir = tree.path("data");
    fs::create_dir(&data_dir).unwrap();
    let stored = json!({"version": 1, "mounts": [], "binds": [
        {"src": tree.path("shared"), "dst": tree.path("h1")},
    ]});
    fs::write(data_dir.join("state.json"), stored.to_string()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vm-herd-fs"))
        .args(["--listen", "127.0.0.1:0", "--data-dir"])
        .arg(&data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut listening = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut listening)
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    let mut reason = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut reason)
        .unwrap();
    assert!(listening.starts_with("listening on"), "{listening}");
    assert!(
        reason.contains(tree.path("h1/tools").to_str().unwrap()),
        "{reason}"
    );
    let after: Value =
        serde_json::from_str(&fs::read_to_string(data_dir.join("state.json")).unwrap()).unwrap();
    assert_eq!(after, stored);
}

#[test]
fn shared_entry_keeps_its_inode_across_a_change_of_host() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.bind_ok(tree.bind_params("h1"));
    daemon.bind_ok(tree.bind_params("h2"));
    daemon.mount_ok(tree.params("h1", "d"));
    let tools = tree.path("d/tools");
    let inode = fs::metadata(&tools).unwrap().ino();
    let stop = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(AtomicUsize::new(0));
    let stat_loop = {
        let (stop, stats) = (stop.clone(), stats.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let meta = fs::metadata(&tools).expect("stat during a change");
                assert_eq!(meta.ino(), inode);
                stats.fetch_add(1, Ordering::SeqCst);
            }
        })
    };
    for round in 0..20 {
        let before = stats.load(Ordering::SeqCst);
        daemon.mount_ok(tree.params(["h2", "h1"][round % 2], "d"));
        while stats.load(Ordering::SeqCst) == before && !stat_loop.is_finished() {
            thread::yield_now();
        }
    }
    stop.store(true, Ordering::SeqCst);
    stat_loop.join().unwrap();
    assert_eq!(fs::metadata(tree.path("d/tools")).unwrap().ino(), inode);
}

fn binds(daemon: &Daemon) -> Value {
    let reply = daemon.call("binds", json!(null));
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

#[test]
fn no_binds() {
    let daemon = Daemon::start();
    assert_eq!(binds(&daemon), json!([]));
}

#[test]
fn bound_host() {
    let (tree, daemon) = bound();
    assert_eq!(
        binds(&daemon),
        json!([{"src": tree.path("shared"), "dst": tree.path("h1")}])
    );
}

#[test]
fn two_hosts() {
    let tree = Tree::new();
    let daemon = Daemon::start();
    daemon.bind_ok(tree.bind_params("h2"));
    daemon.bind_ok(tree.bind_params("h1"));
    assert_eq!(
        binds(&daemon),
        json!([
            {"src": tree.path("shared"), "dst": tree.path("h1")},
            {"src": tree.path("shared"), "dst": tree.path("h2")},
        ])
    );
}

#[test]
fn after_unbind() {
    let (tree, daemon) = bound();
    unbind(&daemon, &tree);
    assert_eq!(binds(&daemon), json!([]));
}

#[test]
fn item_that_failed_at_start() {
    let tree = Tree::new();
    fs::write(tree.path("h1/tools"), "").unwrap();
    let data_dir = tree.path("data");
    fs::create_dir(&data_dir).unwrap();
    let stored = json!({"version": 1, "mounts": [], "binds": [
        {"src": tree.path("shared"), "dst": tree.path("h1")},
    ]});
    fs::write(data_dir.join("state.json"), stored.to_string()).unwrap();
    let daemon = Daemon::start_in(&data_dir);
    assert!(!is_mount_point(&tree.path("h1/notes.md")));
    assert_eq!(
        binds(&daemon),
        json!([{"src": tree.path("shared"), "dst": tree.path("h1")}])
    );
}

#[test]
fn binds_with_unexpected_params() {
    let daemon = Daemon::start();
    let reply = daemon.call("binds", json!({"dst": "/w/h1"}));
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}
