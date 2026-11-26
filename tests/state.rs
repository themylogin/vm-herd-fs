mod common;

use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

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
            "type": "cattle",
            "dst": tree.path("d"),
            "overlay": {
                "individual": {"src": tree.path("overlay"), "directories": [".idea"]},
                "shared": {"src": tree.path("shared")},
            },
            "squash": {"uid": 1000, "gid": 1000},
        }]})
    );
}

#[test]
fn pet_mount() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.pet_params("h1", "d"));
    assert_eq!(
        state(&daemon),
        json!({"version": 1, "mounts": [{
            "type": "pet",
            "dst": tree.path("d"),
            "src": tree.path("h1"),
            "squash": {"uid": 1000, "gid": 1000},
        }]})
    );
}

#[test]
fn cattle_without_overlay_replaces_a_pet() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.pet_params("h1", "d"));
    let mut params = tree.params("h1", "d");
    params["overlay"] = json!(null);
    daemon.mount_ok(params);
    assert_eq!(state(&daemon)["mounts"], json!([]));
}

#[test]
fn restart_with_a_pet() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.pet_params("h1", "d"));
    daemon.restart();
    assert_eq!(names(&tree.path("d")), ["proj1", "proj3"]);
    fs::write(tree.path("d/proj1/main.py"), "after a restart").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("h1/proj1/main.py")).unwrap(),
        "after a restart"
    );
    assert_eq!(
        fs::metadata(tree.path("d/proj1/main.py")).unwrap().uid(),
        1000
    );
    assert_eq!(
        daemon.call("mounts", json!(null))["result"],
        json!([{
            "type": "pet",
            "src": tree.path("h1"),
            "overlay": null,
            "dst": tree.path("d"),
            "squash": {"uid": 1000, "gid": 1000},
        }])
    );
}

#[test]
fn pet_src_missing_at_start() {
    let tree = Tree::new();
    let data_dir = tree.path("data");
    fs::create_dir(&data_dir).unwrap();
    let stored = json!({"version": 1, "mounts": [{
        "type": "pet",
        "dst": tree.path("d"),
        "src": tree.path("gone"),
        "squash": null,
    }]});
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
    let mounted = is_mount_point(&tree.path("d"));
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
    assert!(!mounted);
    assert!(
        reason.contains(tree.path("d").to_str().unwrap()),
        "{reason}"
    );
    assert!(
        reason.contains(tree.path("gone").to_str().unwrap()),
        "{reason}"
    );
    let after: Value =
        serde_json::from_str(&fs::read_to_string(data_dir.join("state.json")).unwrap()).unwrap();
    assert_eq!(after, stored);
}

#[test]
fn restart_with_shared_only() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["overlay"] = json!({"shared": {"src": tree.path("shared")}});
    daemon.mount_ok(params);
    assert_eq!(
        state(&daemon)["mounts"][0]["overlay"],
        json!({"individual": null, "shared": {"src": tree.path("shared")}})
    );
    daemon.restart();
    assert_eq!(names(&tree.path("d")), ["notes.md", "tools"]);
    fs::write(tree.path("d/notes.md"), "after a restart").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("shared/notes.md")).unwrap(),
        "after a restart"
    );
    let err = fs::create_dir(tree.path("d/proj5")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
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
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
    assert_eq!(names(&tree.path("d/proj1")), [".idea"]);
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/.idea/workspace.xml")).unwrap(),
        "proj1"
    );
    assert_eq!(
        fs::read_to_string(tree.path("d/notes.md")).unwrap(),
        "shared notes\n"
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
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
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
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
    assert_eq!(fs_type(&tree.path("d2")).as_deref(), Some("tmpfs"));
    let mounts = state(&daemon)["mounts"].as_array().unwrap().clone();
    assert!(
        mounts
            .iter()
            .any(|item| item["dst"] == json!(tree.path("d2")))
    );
}
