mod common;

use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt, chown};
use std::thread::sleep;
use std::time::Duration;

use common::{Daemon, Tree, as_user, names};
use serde_json::json;

// The view tells the kernel to cache entries and attributes for 1 second.
const PAST_CACHE_TIMEOUT: Duration = Duration::from_millis(1100);

fn mounted(host: &str) -> (Tree, Daemon) {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params(host, "d"));
    (tree, daemon)
}

#[test]
fn union_of_projects() {
    let (tree, _daemon) = mounted("h1");
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
}

#[test]
fn no_overlay() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(
        json!({"src": tree.path("h1"), "overlay": null, "dst": tree.path("d"), "squash": null}),
    );
    assert_eq!(names(&tree.path("d")), ["proj1", "proj3"]);
    assert_eq!(names(&tree.path("d/proj1")), ["main.py"]);
}

#[test]
fn new_project_in_src() {
    let (tree, _daemon) = mounted("h1");
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3"]);
    fs::create_dir(tree.path("h1/proj4")).unwrap();
    fs::write(tree.path("h1/proj4/main.py"), "new").unwrap();
    assert_eq!(names(&tree.path("d")), ["proj1", "proj2", "proj3", "proj4"]);
    assert_eq!(names(&tree.path("d/proj4")), ["main.py"]);
}

#[test]
fn nfs_client_edits_a_file() {
    let (tree, _daemon) = mounted("h1");
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h1 proj1\n"
    );
    fs::write(
        tree.path("h1/proj1/main.py"),
        "edited elsewhere, and longer\n",
    )
    .unwrap();
    sleep(PAST_CACHE_TIMEOUT);
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "edited elsewhere, and longer\n"
    );
}

#[test]
fn nfs_client_renames_an_open_file() {
    let (tree, _daemon) = mounted("h1");
    fs::write(tree.path("h1/proj1/a.py"), "content of a").unwrap();
    let mut open = fs::File::open(tree.path("d/proj1/a.py")).unwrap();
    fs::rename(tree.path("h1/proj1/a.py"), tree.path("h1/proj1/b.py")).unwrap();
    sleep(PAST_CACHE_TIMEOUT);
    assert!(tree.path("d/proj1/b.py").exists());
    assert!(!tree.path("d/proj1/a.py").exists());
    let mut content = String::new();
    open.read_to_string(&mut content).unwrap();
    assert_eq!(content, "content of a");
}

#[test]
fn symlink_read() {
    let (tree, _daemon) = mounted("h1");
    std::os::unix::fs::symlink("main.py", tree.path("h1/proj1/link")).unwrap();
    assert_eq!(
        fs::read_link(tree.path("d/proj1/link")).unwrap().to_str(),
        Some("main.py")
    );
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/link")).unwrap(),
        "h1 proj1\n"
    );
}

#[test]
fn source_has_its_own_idea() {
    let tree = Tree::new();
    fs::create_dir(tree.path("h1/proj1/.idea")).unwrap();
    fs::write(tree.path("h1/proj1/.idea/other.xml"), "source").unwrap();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(names(&tree.path("d/proj1/.idea")), ["workspace.xml"]);
    assert_eq!(names(&tree.path("d/proj1")), [".idea", "main.py"]);
}

#[test]
fn source_has_no_idea() {
    let (tree, _daemon) = mounted("h2");
    assert_eq!(names(&tree.path("d/proj2")), [".idea", "main.py"]);
    assert!(!tree.path("h2/proj2/.idea").exists());
}

#[test]
fn new_project_has_no_overlay_directory() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir(tree.path("h1/proj4")).unwrap();
    assert!(names(&tree.path("d/proj4")).is_empty());
    assert!(!tree.path("d/proj4/.idea").exists());
    assert!(!tree.path("overlay/proj4").exists());
}

#[test]
fn source_idea_stays_hidden_without_overlay_directory() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir_all(tree.path("h1/proj4/.idea")).unwrap();
    fs::write(tree.path("h1/proj4/main.py"), "").unwrap();
    assert_eq!(names(&tree.path("d/proj4")), ["main.py"]);
    assert!(!tree.path("d/proj4/.idea").exists());
}

#[test]
fn create_idea_in_a_new_project() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir(tree.path("h1/proj4")).unwrap();
    fs::create_dir(tree.path("d/proj4/.idea")).unwrap();
    fs::write(tree.path("d/proj4/.idea/workspace.xml"), "new").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("overlay/proj4/.idea/workspace.xml")).unwrap(),
        "new"
    );
    assert!(!tree.path("h1/proj4/.idea").exists());
    assert_eq!(names(&tree.path("d/proj4")), [".idea"]);
}

#[test]
fn create_idea_in_a_read_only_project() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir(tree.path("overlay/proj6")).unwrap();
    assert!(names(&tree.path("d/proj6")).is_empty());
    fs::create_dir(tree.path("d/proj6/.idea")).unwrap();
    assert!(tree.path("overlay/proj6/.idea").is_dir());
    assert_eq!(names(&tree.path("d/proj6")), [".idea"]);
}

#[test]
fn overlay_in_a_read_only_project() {
    let (tree, _daemon) = mounted("h1");
    fs::write(tree.path("d/proj2/.idea/workspace.xml"), "saved by the IDE").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("overlay/proj2/.idea/workspace.xml")).unwrap(),
        "saved by the IDE"
    );
}

#[test]
fn write_through_the_view() {
    let (tree, _daemon) = mounted("h1");
    fs::write(tree.path("d/proj1/main.py"), "written through the view").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("h1/proj1/main.py")).unwrap(),
        "written through the view"
    );
}

#[test]
fn new_project_through_the_view() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir(tree.path("d/proj5")).unwrap();
    assert!(tree.path("h1/proj5").is_dir());
    assert!(names(&tree.path("d/proj5")).is_empty());
    fs::write(tree.path("d/root.txt"), "at the root").unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("h1/root.txt")).unwrap(),
        "at the root"
    );
}

#[test]
fn rename_inside_a_project() {
    let (tree, _daemon) = mounted("h1");
    fs::create_dir(tree.path("d/proj1/pkg")).unwrap();
    fs::rename(
        tree.path("d/proj1/main.py"),
        tree.path("d/proj1/pkg/app.py"),
    )
    .unwrap();
    assert!(!tree.path("h1/proj1/main.py").exists());
    assert_eq!(
        fs::read_to_string(tree.path("h1/proj1/pkg/app.py")).unwrap(),
        "h1 proj1\n"
    );
    fs::remove_file(tree.path("d/proj1/pkg/app.py")).unwrap();
    fs::remove_dir(tree.path("d/proj1/pkg")).unwrap();
    assert!(!tree.path("h1/proj1/pkg").exists());
}

#[test]
fn file_with_another_owner() {
    let (tree, _daemon) = mounted("h1");
    chown(tree.path("h1/proj1/main.py"), Some(1001), Some(1001)).unwrap();
    fs::set_permissions(
        tree.path("h1/proj1/main.py"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let meta = fs::metadata(tree.path("d/proj1/main.py")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (1000, 1000));
    let path = tree.path("d/proj1/main.py");
    let output = as_user(1000, &format!("echo appended >> {}", path.display()));
    assert!(output.status.success(), "{output:?}");
    assert!(
        fs::read_to_string(tree.path("h1/proj1/main.py"))
            .unwrap()
            .ends_with("appended\n")
    );
}

#[test]
fn file_created_with_squash() {
    let (tree, _daemon) = mounted("h1");
    let output = as_user(
        1000,
        &format!("touch {}", tree.path("d/proj1/new.py").display()),
    );
    assert!(output.status.success(), "{output:?}");
    let meta = fs::metadata(tree.path("h1/proj1/new.py")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (0, 0));
}

#[test]
fn root_runs_chown_with_squash() {
    let (tree, _daemon) = mounted("h1");
    fs::write(tree.path("d/proj1/new.py"), "").unwrap();
    chown(tree.path("d/proj1/new.py"), Some(1234), Some(1234)).unwrap();
    let meta = fs::metadata(tree.path("h1/proj1/new.py")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (0, 0));
}

#[test]
fn file_created_without_squash() {
    let tree = Tree::new();
    fs::set_permissions(tree.path("h1/proj1"), fs::Permissions::from_mode(0o777)).unwrap();
    let mut daemon = Daemon::start();
    let mut params = tree.params("h1", "d");
    params["squash"] = json!(null);
    daemon.mount_ok(params);
    let output = as_user(
        1000,
        &format!("touch {}", tree.path("d/proj1/new.py").display()),
    );
    assert!(output.status.success(), "{output:?}");
    let meta = fs::metadata(tree.path("h1/proj1/new.py")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (1000, 1000));
    chown(tree.path("d/proj1/new.py"), Some(1234), Some(1234)).unwrap();
    let meta = fs::metadata(tree.path("h1/proj1/new.py")).unwrap();
    assert_eq!((meta.uid(), meta.gid()), (1234, 1234));
}

#[test]
fn project_missing_in_src() {
    let (tree, _daemon) = mounted("h1");
    assert_eq!(names(&tree.path("d/proj2")), [".idea"]);
}

#[test]
fn write_into_a_read_only_project() {
    let (tree, _daemon) = mounted("h1");
    let err = fs::write(tree.path("d/proj2/new.txt"), "x").unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EROFS));
    let output = as_user(
        1000,
        &format!("touch {}", tree.path("d/proj2/new.txt").display()),
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Read-only file system"),
        "{output:?}"
    );
    assert!(!tree.path("overlay/proj2/new.txt").exists());
}

#[test]
fn remove_an_overlay_directory() {
    let (tree, _daemon) = mounted("h1");
    let err = fs::rename(tree.path("d/proj1/.idea"), tree.path("d/proj1/idea-old")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EPERM));
    let err = fs::remove_dir(tree.path("d/proj2")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EPERM));
    let err = fs::rename(tree.path("d/proj2"), tree.path("d/proj9")).unwrap_err();
    assert_eq!(err.raw_os_error(), Some(libc::EPERM));
    assert!(tree.path("overlay/proj1/.idea").is_dir());
}
