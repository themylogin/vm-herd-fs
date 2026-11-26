mod common;

use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use common::{Daemon, Tree, names};

#[test]
fn change_of_host() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h1 proj1\n"
    );
    assert_eq!(names(&tree.path("d/proj3")), [".idea", "main.py"]);
    daemon.mount_ok(tree.params("h2", "d"));
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h2 proj1\n"
    );
    assert_eq!(names(&tree.path("d/proj2")), [".idea", "main.py"]);
    assert_eq!(names(&tree.path("d/proj3")), [".idea"]);
    assert_eq!(
        names(&tree.path("d")),
        ["notes.md", "proj1", "proj2", "proj3", "tools"]
    );
}

#[test]
fn shared_entry_keeps_its_inode() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
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

#[test]
fn shell_inside_a_project() {
    let tree = Tree::new();
    fs::write(tree.path("h2/proj1/only_on_h2.py"), "").unwrap();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let mut shell = Command::new("sh")
        .args(["-c", "read line; ls -A"])
        .current_dir(tree.path("d/proj1"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    daemon.mount_ok(tree.params("h2", "d"));
    shell.stdin.take().unwrap().write_all(b"\n").unwrap();
    let output = shell.wait_with_output().unwrap();
    let listing = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        listing.lines().collect::<Vec<_>>(),
        [".idea", "main.py", "only_on_h2.py"]
    );
}

#[test]
fn two_views() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    daemon.mount_ok(tree.params("h2", "d2"));
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h1 proj1\n"
    );
    assert_eq!(
        fs::read_to_string(tree.path("d2/proj1/main.py")).unwrap(),
        "h2 proj1\n"
    );
}

#[test]
fn squash_change_applies_at_once() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    assert_eq!(
        fs::metadata(tree.path("d/proj1/main.py")).unwrap().uid(),
        1000
    );
    let mut params = tree.params("h1", "d");
    params["squash"]["uid"] = 2000.into();
    daemon.mount_ok(params);
    assert_eq!(
        fs::metadata(tree.path("d/proj1/main.py")).unwrap().uid(),
        2000
    );
}

#[test]
fn concurrent_lookups_during_a_change() {
    let tree = Tree::new();
    let mut daemon = Daemon::start();
    daemon.mount_ok(tree.params("h1", "d"));
    let idea = tree.path("d/proj1/.idea");
    let inode = fs::metadata(&idea).unwrap().ino();
    let stop = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(AtomicUsize::new(0));
    let creates = Arc::new(AtomicUsize::new(0));

    let stat_loop = {
        let (stop, stats) = (stop.clone(), stats.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let meta = fs::metadata(&idea).expect("stat during a change");
                assert_eq!(meta.ino(), inode);
                stats.fetch_add(1, Ordering::SeqCst);
            }
        })
    };
    let create_loop = {
        let (stop, creates) = (stop.clone(), creates.clone());
        let project = tree.path("d/proj1");
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let n = creates.load(Ordering::SeqCst);
                fs::write(project.join(format!("file{n}")), "").expect("create during a change");
                creates.fetch_add(1, Ordering::SeqCst);
            }
        })
    };

    for round in 0..50 {
        let (stats_before, creates_before) =
            (stats.load(Ordering::SeqCst), creates.load(Ordering::SeqCst));
        let host = ["h2", "h1"][round % 2];
        daemon.mount_ok(tree.params(host, "d"));
        while stats.load(Ordering::SeqCst) == stats_before
            || creates.load(Ordering::SeqCst) == creates_before
        {
            if stat_loop.is_finished() || create_loop.is_finished() {
                break;
            }
            thread::yield_now();
        }
    }
    stop.store(true, Ordering::SeqCst);
    stat_loop.join().unwrap();
    create_loop.join().unwrap();
    assert_eq!(
        fs::read_to_string(tree.path("d/proj1/main.py")).unwrap(),
        "h1 proj1\n"
    );
}
