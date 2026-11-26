use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::OverlayParams;
use crate::view::Squash;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoredMount {
    pub dst: PathBuf,
    pub overlay: OverlayParams,
    pub squash: Option<Squash>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    mounts: Vec<StoredMount>,
}

#[derive(Deserialize)]
struct Version {
    version: u32,
}

pub fn load(data_dir: &Path) -> Result<Vec<StoredMount>, String> {
    let path = data_dir.join("state.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };
    let Version { version } =
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    if version != 1 {
        return Err(format!(
            "{}: version {version} is not supported",
            path.display()
        ));
    }
    let state: State =
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(state.mounts)
}

/// Writes a temporary file and renames it over `state.json`, so a crash never leaves a
/// partly written file.
pub fn save(data_dir: &Path, mounts: Vec<StoredMount>) -> io::Result<()> {
    let state = State { version: 1, mounts };
    let temporary = data_dir.join("state.json.tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(&state)?)?;
    file.sync_all()?;
    std::fs::rename(&temporary, data_dir.join("state.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, text: &str) {
        std::fs::write(dir.join("state.json"), text).unwrap();
    }

    #[test]
    fn missing_file_means_no_mounts() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn version_1() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "squash": {"uid": 1000, "gid": 1000}}]}"#,
        );
        let mounts = load(dir.path()).unwrap();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].dst, Path::new("/d"));
        assert_eq!(mounts[0].overlay.directories, [".idea"]);
        assert_eq!(
            mounts[0].squash,
            Some(Squash {
                uid: 1000,
                gid: 1000
            })
        );
    }

    #[test]
    fn version_2() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), r#"{"version": 2, "mounts": {"changed": true}}"#);
        let err = load(dir.path()).err().unwrap();
        assert!(err.ends_with("version 2 is not supported"), "{err}");
    }

    #[test]
    fn invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "{not json");
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn save_then_load() {
        let dir = tempfile::tempdir().unwrap();
        let mount = StoredMount {
            dst: "/d".into(),
            overlay: OverlayParams {
                src: "/w/overlay".into(),
                directories: vec![".idea".into()],
            },
            squash: None,
        };
        save(dir.path(), vec![mount]).unwrap();
        assert!(!dir.path().join("state.json.tmp").exists());
        let mounts = load(dir.path()).unwrap();
        assert_eq!(mounts[0].dst, Path::new("/d"));
        assert_eq!(mounts[0].squash, None);
    }
}
