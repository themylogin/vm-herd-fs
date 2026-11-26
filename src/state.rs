use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::OverlayParams;
use crate::view::Squash;

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum StoredMount {
    Cattle {
        dst: PathBuf,
        overlay: OverlayParams,
        squash: Option<Squash>,
    },
    Pet {
        dst: PathBuf,
        src: PathBuf,
        squash: Option<Squash>,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoredBind {
    pub src: PathBuf,
    pub dst: PathBuf,
}

impl StoredMount {
    pub fn dst(&self) -> &Path {
        match self {
            Self::Cattle { dst, .. } | Self::Pet { dst, .. } => dst,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    mounts: Vec<StoredMount>,
    binds: Vec<StoredBind>,
}

#[derive(Deserialize)]
struct Version {
    version: u32,
}

pub fn load(data_dir: &Path) -> Result<(Vec<StoredMount>, Vec<StoredBind>), String> {
    let path = data_dir.join("state.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok((vec![], vec![])),
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
    Ok((state.mounts, state.binds))
}

/// Writes a temporary file and renames it over `state.json`, so a crash never leaves a
/// partly written file.
pub fn save(data_dir: &Path, mounts: Vec<StoredMount>, binds: Vec<StoredBind>) -> io::Result<()> {
    let state = State {
        version: 1,
        mounts,
        binds,
    };
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
        let (mounts, binds) = load(dir.path()).unwrap();
        assert!(mounts.is_empty());
        assert!(binds.is_empty());
    }

    #[test]
    fn cattle_item() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"src": "/w/overlay", "directories": [".idea"]}, "squash": {"uid": 1000, "gid": 1000}}], "binds": []}"#,
        );
        let (mounts, _) = load(dir.path()).unwrap();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].dst(), Path::new("/d"));
        let StoredMount::Cattle {
            overlay, squash, ..
        } = &mounts[0]
        else {
            panic!("not a cattle item");
        };
        assert_eq!(overlay.src, Path::new("/w/overlay"));
        assert_eq!(overlay.directories, [".idea"]);
        assert_eq!(
            *squash,
            Some(Squash {
                uid: 1000,
                gid: 1000
            })
        );
    }

    #[test]
    fn pet_item() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "squash": null}], "binds": []}"#,
        );
        let (mounts, _) = load(dir.path()).unwrap();
        assert_eq!(mounts[0].dst(), Path::new("/d"));
        let StoredMount::Pet { src, squash, .. } = &mounts[0] else {
            panic!("not a pet item");
        };
        assert_eq!(src, Path::new("/w/h1"));
        assert_eq!(*squash, None);
    }

    #[test]
    fn pet_item_with_overlay() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"type": "pet", "dst": "/d", "src": "/w/h1", "overlay": null, "squash": null}], "binds": []}"#,
        );
        let err = load(dir.path()).err().unwrap();
        assert!(err.contains("unknown field `overlay`"), "{err}");
    }

    #[test]
    fn file_without_binds() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), r#"{"version": 1, "mounts": []}"#);
        let err = load(dir.path()).err().unwrap();
        assert!(err.contains("`binds`"), "{err}");
    }

    #[test]
    fn item_without_type() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"dst": "/d", "src": "/w/h1", "squash": null}], "binds": []}"#,
        );
        let err = load(dir.path()).err().unwrap();
        assert!(err.contains("`type`"), "{err}");
    }

    #[test]
    fn cattle_item_with_src() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "src": "/w/h1", "overlay": {"src": "/w/overlay", "directories": []}, "squash": null}], "binds": []}"#,
        );
        let err = load(dir.path()).err().unwrap();
        assert!(err.contains("unknown field `src`"), "{err}");
    }

    #[test]
    fn overlay_with_members() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"version": 1, "mounts": [{"type": "cattle", "dst": "/d", "overlay": {"individual": {"src": "/w/overlay", "directories": [".idea"]}}, "squash": null}], "binds": []}"#,
        );
        let err = load(dir.path()).err().unwrap();
        assert!(err.contains("unknown field `individual`"), "{err}");
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
        let cattle = StoredMount::Cattle {
            dst: "/d".into(),
            overlay: OverlayParams {
                src: "/w/overlay".into(),
                directories: vec![".idea".into()],
            },
            squash: None,
        };
        let pet = StoredMount::Pet {
            dst: "/d2".into(),
            src: "/w/h1".into(),
            squash: Some(Squash {
                uid: 1000,
                gid: 1000,
            }),
        };
        let bind = StoredBind {
            src: "/w/shared".into(),
            dst: "/w/h1".into(),
        };
        save(dir.path(), vec![cattle, pet], vec![bind]).unwrap();
        assert!(!dir.path().join("state.json.tmp").exists());
        let text = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
        assert!(text.contains(r#""type": "cattle""#), "{text}");
        assert!(text.contains(r#""type": "pet""#), "{text}");
        let (mounts, binds) = load(dir.path()).unwrap();
        assert_eq!(mounts[0].dst(), Path::new("/d"));
        assert!(matches!(&mounts[1], StoredMount::Pet { src, .. } if src == Path::new("/w/h1")));
        assert_eq!(binds[0].src, Path::new("/w/shared"));
        assert_eq!(binds[0].dst, Path::new("/w/h1"));
    }
}
