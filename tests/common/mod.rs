#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

pub struct Daemon {
    child: Child,
    port: u16,
    data_dir: PathBuf,
    mounted: Vec<PathBuf>,
    _own_data_dir: Option<TempDir>,
}

impl Daemon {
    pub fn start() -> Self {
        let own = tempfile::tempdir().unwrap();
        let mut daemon = Self::start_in(own.path());
        daemon._own_data_dir = Some(own);
        daemon
    }

    pub fn start_in(data_dir: &Path) -> Self {
        let (child, port) = spawn(data_dir);
        Self {
            child,
            port,
            data_dir: data_dir.to_owned(),
            mounted: vec![],
            _own_data_dir: None,
        }
    }

    /// Kills the daemon with SIGKILL, which leaves its mounts dead, and starts it again on
    /// the same data directory.
    pub fn restart(&mut self) {
        self.kill();
        let (child, port) = spawn(&self.data_dir);
        self.child = child;
        self.port = port;
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn track(&mut self, dst: &Path) {
        self.mounted.push(dst.to_owned());
    }

    pub fn post(&self, path: &str, body: &str) -> (u16, String) {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent();
        let mut response = agent
            .post(format!("http://127.0.0.1:{}{path}", self.port))
            .content_type("application/json")
            .send(body)
            .unwrap();
        let status = response.status().as_u16();
        (status, response.body_mut().read_to_string().unwrap())
    }

    pub fn get(&self, path: &str) -> u16 {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent();
        let response = agent
            .get(format!("http://127.0.0.1:{}{path}", self.port))
            .call()
            .unwrap();
        response.status().as_u16()
    }

    pub fn call(&self, method: &str, params: Value) -> Value {
        let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1});
        let (status, reply) = self.post("/api", &body.to_string());
        assert_eq!(status, 200, "{reply}");
        serde_json::from_str(&reply).unwrap()
    }

    pub fn mount(&mut self, params: Value) -> Value {
        let reply = self.call("mount", params.clone());
        if reply.get("error").is_none() {
            self.mounted
                .push(PathBuf::from(params["dst"].as_str().unwrap()));
        }
        reply
    }

    pub fn mount_ok(&mut self, params: Value) {
        let reply = self.mount(params);
        assert_eq!(reply["result"], Value::Null, "{reply}");
        assert!(reply.get("error").is_none(), "{reply}");
    }

    pub fn bind_ok(&self, params: Value) {
        let reply = self.call("bind", params);
        assert_eq!(reply["result"], Value::Null, "{reply}");
        assert!(reply.get("error").is_none(), "{reply}");
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn(data_dir: &Path) -> (Child, u16) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vm-herd-fs"))
        .args(["--listen", "127.0.0.1:0", "--data-dir"])
        .arg(data_dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port = line
        .trim()
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .unwrap_or_else(|| panic!("no port in {line:?}"));
    (child, port)
}

pub fn run_daemon(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vm-herd-fs"))
        .args(args)
        .output()
        .unwrap()
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.kill();
        for dst in &self.mounted {
            while is_mount_point(dst) {
                let detached = Command::new("umount").arg("-l").arg(dst).status();
                if !detached.is_ok_and(|status| status.success()) {
                    break;
                }
            }
        }
    }
}

pub struct Tree {
    pub dir: TempDir,
}

/// Bind mounts in `src` outlive the daemon, and a mount point blocks the removal of the
/// temporary directory.
impl Drop for Tree {
    fn drop(&mut self) {
        let root = self.dir.path().to_str().unwrap().to_owned();
        loop {
            let mut points: Vec<String> = fs::read_to_string("/proc/self/mountinfo")
                .unwrap()
                .lines()
                .filter_map(|line| line.split(' ').nth(4))
                .filter(|point| point.starts_with(&format!("{root}/")))
                .map(str::to_owned)
                .collect();
            if points.is_empty() {
                break;
            }
            points.sort_by_key(|point| std::cmp::Reverse(point.len()));
            let detached = points.iter().filter(|point| {
                Command::new("umount")
                    .arg("-l")
                    .arg(point)
                    .status()
                    .is_ok_and(|status| status.success())
            });
            if detached.count() == 0 {
                break;
            }
        }
    }
}

impl Tree {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let tree = Self { dir };
        for (host, project) in [
            ("h1", "proj1"),
            ("h1", "proj3"),
            ("h2", "proj1"),
            ("h2", "proj2"),
        ] {
            let path = tree.path(&format!("{host}/{project}"));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("main.py"), format!("{host} {project}\n")).unwrap();
        }
        for project in ["proj1", "proj2", "proj3"] {
            let idea = tree.path(&format!("overlay/{project}/.idea"));
            fs::create_dir_all(&idea).unwrap();
            fs::write(idea.join("workspace.xml"), project).unwrap();
        }
        fs::create_dir_all(tree.path("shared/tools")).unwrap();
        fs::write(tree.path("shared/notes.md"), "shared notes\n").unwrap();
        fs::write(tree.path("shared/tools/run.sh"), "#!/bin/sh\n").unwrap();
        fs::create_dir_all(tree.path("d")).unwrap();
        fs::create_dir_all(tree.path("d2")).unwrap();
        tree
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    pub fn params(&self, host: &str, dst: &str) -> Value {
        json!({
            "type": "cattle",
            "src": self.path(host),
            "overlay": {"src": self.path("overlay"), "directories": [".idea"]},
            "dst": self.path(dst),
            "squash": {"uid": 1000, "gid": 1000},
        })
    }
}

impl Tree {
    pub fn pet_params(&self, host: &str, dst: &str) -> Value {
        json!({
            "type": "pet",
            "src": self.path(host),
            "dst": self.path(dst),
            "squash": {"uid": 1000, "gid": 1000},
        })
    }

    pub fn bind_params(&self, host: &str) -> Value {
        json!({"src": self.path("shared"), "dst": self.path(host)})
    }
}

pub fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

pub fn mount_count(path: &Path) -> usize {
    fs::read_to_string("/proc/self/mountinfo")
        .unwrap()
        .lines()
        .filter(|line| line.split(' ').nth(4) == Some(path.to_str().unwrap()))
        .count()
}

pub fn is_mount_point(path: &Path) -> bool {
    fs::read_to_string("/proc/self/mountinfo")
        .unwrap()
        .lines()
        .any(|line| line.split(' ').nth(4) == Some(path.to_str().unwrap()))
}

pub fn as_user(uid: u32, script: &str) -> Output {
    Command::new("setpriv")
        .args([
            &format!("--reuid={uid}"),
            &format!("--regid={uid}"),
            "--clear-groups",
            "sh",
            "-c",
            script,
        ])
        .output()
        .unwrap()
}
