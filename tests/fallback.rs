mod common;

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use common::{Daemon, Tree, as_user, names};
use serde_json::json;

// The view tells the kernel to cache entries and attributes for 1 second.
const PAST_CACHE_TIMEOUT: Duration = Duration::from_millis(1100);

fn mounted() -> (Tree, Daemon) {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.fallback_params("h1", "d"));
    (tree, daemon)
}

#[test]
fn project_without_git() {
    let (tree, _daemon) = mounted();
    assert_eq!(names(&tree.path("d/proj1")), [".git", ".idea", "main.py"]);
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/.git/HEAD")).unwrap(),
        "ref: refs/heads/main"
    );
}

#[test]
fn git_of_src_wins() {
    let tree = Tree::new();
    fs::create_dir(tree.path("h1/proj1/.git")).unwrap();
    fs::write(tree.path("h1/proj1/.git/config"), "").unwrap();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.fallback_params("h1", "d"));
    assert_eq!(names(&tree.path("d/proj1/.git")), ["config"]);
}

#[test]
fn overlay_name_wins() {
    let tree = Tree::new();
    fs::create_dir(tree.path("fallback/.idea")).unwrap();
    fs::write(tree.path("fallback/.idea/other.xml"), "").unwrap();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.fallback_params("h1", "d"));
    assert_eq!(names(&tree.path("d/proj1/.idea")), ["workspace.xml"]);
}

#[test]
fn project_missing_in_src() {
    let (tree, _daemon) = mounted();
    assert_eq!(names(&tree.path("d/proj2")), [".git", ".idea"]);
}

#[test]
fn new_entry_in_the_fallback_store() {
    let (tree, _daemon) = mounted();
    assert_eq!(names(&tree.path("d/proj1")), [".git", ".idea", "main.py"]);
    fs::write(tree.path("fallback/.envrc"), "").unwrap();
    sleep(PAST_CACHE_TIMEOUT);
    assert_eq!(
        names(&tree.path("d/proj1")),
        [".envrc", ".git", ".idea", "main.py"]
    );
    assert_eq!(
        names(&tree.path("d/proj3")),
        [".envrc", ".git", ".idea", "main.py"]
    );
}

#[test]
fn git_appears_in_src() {
    let (tree, _daemon) = mounted();
    assert_eq!(names(&tree.path("d/proj3/.git")), ["HEAD"]);
    fs::create_dir(tree.path("h1/proj3/.git")).unwrap();
    fs::write(tree.path("h1/proj3/.git/config"), "").unwrap();
    sleep(PAST_CACHE_TIMEOUT);
    assert_eq!(names(&tree.path("d/proj3/.git")), ["config"]);
}

#[test]
fn no_fallback() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    let mut params = tree.fallback_params("h1", "d");
    params["fallback"] = json!(null);
    daemon.mount_ok(params);
    assert_eq!(names(&tree.path("d/proj1")), [".idea", "main.py"]);
}

fn assert_store_unchanged(tree: &Tree) {
    assert_eq!(names(&tree.path("fallback/.git")), ["HEAD"]);
    assert_eq!(
        fs::read_to_string(tree.path("fallback/.git/HEAD")).unwrap(),
        "ref: refs/heads/main"
    );
}

#[test]
fn write_into_a_fallback_file() {
    let (tree, _daemon) = mounted();
    let path = tree.path("d/proj1/.git/HEAD");
    let output = as_user(1000, &format!("echo other > {}", path.display()));
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Read-only file system"), "{stderr}");
    assert_store_unchanged(&tree);
}

#[test]
fn create_inside_a_fallback_directory() {
    let (tree, _daemon) = mounted();
    let err = fs::File::create(tree.path("d/proj1/.git/index.lock")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
    assert_store_unchanged(&tree);
}

#[test]
fn remove_a_fallback_entry() {
    let (tree, _daemon) = mounted();
    let output = Command::new("rm")
        .arg("-rf")
        .arg(tree.path("d/proj1/.git"))
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Read-only file system"), "{stderr}");
    assert_store_unchanged(&tree);
}

#[test]
fn chmod_with_squash() {
    let (tree, _daemon) = mounted();
    let err = fs::set_permissions(
        tree.path("d/proj1/.git/HEAD"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
    assert_eq!(
        fs::metadata(tree.path("fallback/.git/HEAD"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn read_a_fallback_file() {
    let (tree, _daemon) = mounted();
    let path = tree.path("d/proj1/.git/HEAD");
    let output = as_user(1000, &format!("cat {}", path.display()));
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ref: refs/heads/main"
    );
}

fn inode(tree: &Tree, path: &str) -> u64 {
    fs::metadata(tree.path(path)).unwrap().ino()
}

#[test]
fn two_projects() {
    let (tree, _daemon) = mounted();
    assert_ne!(inode(&tree, "d/proj1/.git"), inode(&tree, "d/proj3/.git"));
    assert_ne!(
        inode(&tree, "d/proj1/.git/HEAD"),
        inode(&tree, "d/proj3/.git/HEAD")
    );
}

#[test]
fn change_of_host() {
    let (tree, mut daemon) = mounted();
    let before = inode(&tree, "d/proj1/.git");
    daemon.mount_ok(tree.fallback_params("h2", "d"));
    assert_eq!(inode(&tree, "d/proj1/.git"), before);
}

#[test]
fn back_to_the_restored_view() {
    let (tree, daemon) = mounted();
    let before = inode(&tree, "d/proj1/.git");
    let reply = daemon.call("umount", json!({"dst": tree.path("d")}));
    assert_eq!(reply["result"], json!(null), "{reply}");
    assert_eq!(names(&tree.path("d/proj1")), [".git", ".idea"]);
    assert_eq!(inode(&tree, "d/proj1/.git"), before);
}
