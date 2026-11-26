mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use common::{Daemon, Tree, is_mount_point, names, run_daemon};
use serde_json::{Value, json};

fn state(daemon: &Daemon) -> Value {
    serde_json::from_str(&fs::read_to_string(daemon.data_dir().join("state.json")).unwrap())
        .unwrap()
}

fn fs_type(path: &Path) -> Option<String> {
    fs::read_to_string("/proc/self/mountinfo")
        .unwrap()
        .lines()
        .rfind(|line| line.split(' ').nth(4) == Some(path.to_str().unwrap()))
        .and_then(|line| line.split_once(" - "))
        .and_then(|(_, fs)| fs.split(' ').next().map(str::to_owned))
}

#[test]
fn missing_data_directory() {
    let tree = Tree::new();
    let data_dir = tree.path("var/lib/vm-herd-fs");
    let daemon = Daemon::start_in(&data_dir);
    assert!(data_dir.is_dir());
    assert_eq!(daemon.call("none", json!(null))["error"]["code"], -32601);
}

#[test]
fn no_data_directory_flag() {
    let output = run_daemon(&[OsStr::new("--listen"), OsStr::new("127.0.0.1:0")]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--data-dir"));
}

#[test]
fn unknown_version() {
    let tree = Tree::new();
    let data_dir = tree.path("data");
    fs::create_dir(&data_dir).unwrap();
    fs::write(
        data_dir.join("state.json"),
        r#"{"version": 2, "mounts": []}"#,
    )
    .unwrap();
    let output = run_daemon(&[
        OsStr::new("--listen"),
        OsStr::new("127.0.0.1:0"),
        OsStr::new("--data-dir"),
        data_dir.as_os_str(),
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("version 2 is not supported"), "{stderr}");
}

#[test]
fn mount_with_overlay() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(
        state(&daemon),
        json!({"version": 1, "mounts": [{
            "dst": tree.path("d"),
            "overlay": {"src": tree.path("overlay"), "directories": [".idea"]},
            "squash": {"uid": 1000, "gid": 1000},
        }]})
    );
}

#[test]
fn change_of_host_keeps_one_item() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h2", "d"));
    let mounts = state(&daemon)["mounts"].as_array().unwrap().clone();
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0]["dst"], json!(tree.path("d")));
}

#[test]
fn mount_without_overlay() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h1", "d2"));
    let mut params = tree.params("h1", "d");
    params["overlay"] = json!(null);
    daemon.mount_ok(params);
    let mounts = state(&daemon)["mounts"].as_array().unwrap().clone();
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0]["dst"], json!(tree.path("d2")));
}

#[test]
fn failed_mount() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let before = fs::read(daemon.data_dir().join("state.json")).unwrap();
    let mut params = tree.params("h1", "d2");
    params["src"] = json!("w/h1");
    assert_eq!(daemon.mount(params)["error"]["code"], -32602);
    assert_eq!(
        fs::read(daemon.data_dir().join("state.json")).unwrap(),
        before
    );
}

#[test]
fn restart_with_an_overlay() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.kill();
    let status = Command::new("umount")
        .arg("-l")
        .arg(tree.path("d"))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!is_mount_point(&tree.path("d")));
    daemon.restart();
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
    assert_eq!(names(&tree.path("d/proj1")), [".idea"]);
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/.idea/workspace.xml")).unwrap(),
        "proj1"
    );
    assert_eq!(
        fs::metadata(tree.path("d/proj1/.idea")).unwrap().uid(),
        1000
    );
}

#[test]
fn restart_after_a_crash() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.restart();
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
    assert_eq!(names(&tree.path("d/proj3")), [".idea"]);
    let mounts = fs::read_to_string("/proc/self/mountinfo").unwrap();
    let at_dst = mounts
        .lines()
        .filter(|line| line.split(' ').nth(4) == Some(tree.path("d").to_str().unwrap()))
        .count();
    assert_eq!(at_dst, 1);
}

#[test]
fn create_at_the_root_of_a_restored_view() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.restart();
    let err = fs::create_dir(tree.path("d/proj5")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
    let err = fs::write(tree.path("d/proj1/main.py"), "").unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
}

#[test]
fn mount_after_a_restore() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.restart();
    let inode = fs::metadata(tree.path("d/proj1/.idea")).unwrap().ino();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(names(&tree.path("d/proj1")), [".idea", "main.py"]);
    assert_eq!(
        fs::metadata(tree.path("d/proj1/.idea")).unwrap().ino(),
        inode
    );
}

#[test]
fn foreign_mount_at_a_stored_dst() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h2", "d2"));
    daemon.kill();
    for dst in ["d", "d2"] {
        assert!(
            Command::new("umount")
                .arg("-l")
                .arg(tree.path(dst))
                .status()
                .unwrap()
                .success()
        );
    }
    let status = Command::new("mount")
        .args(["-t", "tmpfs", "tmpfs"])
        .arg(tree.path("d2"))
        .status()
        .unwrap();
    assert!(status.success());
    daemon.restart();
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
    assert_eq!(fs_type(&tree.path("d2")).as_deref(), Some("tmpfs"));
    let mounts = state(&daemon)["mounts"].as_array().unwrap().clone();
    assert!(
        mounts
            .iter()
            .any(|item| item["dst"] == json!(tree.path("d2")))
    );
}
