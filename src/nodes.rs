use std::collections::HashMap;
use std::ffi::OsString;
use std::os::fd::OwnedFd;
use std::sync::Arc;

pub const ROOT_ID: u64 = 1;

/// `(realm, dev, ino)`. The realm is 0, or the project id for the entries of a substitute
/// `.git`, so that each project gets its own inodes for them.
pub type Key = (u64, u64, u64);

pub enum Node {
    Root,
    Project(OsString),
    Real(Arc<OwnedFd>, u64),
}

struct Real {
    fd: Arc<OwnedFd>,
    realm: u64,
    lookups: u64,
}

pub struct Nodes {
    next_id: u64,
    ids: HashMap<Key, u64>,
    project_ids: HashMap<OsString, u64>,
    project_names: HashMap<u64, OsString>,
    real: HashMap<u64, Real>,
}

impl Default for Nodes {
    fn default() -> Self {
        Self {
            next_id: ROOT_ID + 1,
            ids: HashMap::new(),
            project_ids: HashMap::new(),
            project_names: HashMap::new(),
            real: HashMap::new(),
        }
    }
}

impl Nodes {
    pub fn get(&self, id: u64) -> Option<Node> {
        if id == ROOT_ID {
            return Some(Node::Root);
        }
        if let Some(name) = self.project_names.get(&id) {
            return Some(Node::Project(name.clone()));
        }
        self.real
            .get(&id)
            .map(|real| Node::Real(real.fd.clone(), real.realm))
    }

    pub fn id_for_key(&mut self, key: Key) -> u64 {
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = self.allocate();
        self.ids.insert(key, id);
        id
    }

    pub fn project_id(&mut self, name: &OsString) -> u64 {
        if let Some(id) = self.project_ids.get(name) {
            return *id;
        }
        let id = self.allocate();
        self.project_ids.insert(name.clone(), id);
        self.project_names.insert(id, name.clone());
        id
    }

    pub fn project_ids(&self) -> Vec<u64> {
        self.project_names.keys().copied().collect()
    }

    pub fn real_ids(&self) -> Vec<u64> {
        self.real.keys().copied().collect()
    }

    /// Counts one kernel lookup of the node for `key`. The kernel reports each lookup back
    /// through `forget`, so the descriptor stays open until the count returns to zero.
    pub fn remember(&mut self, key: Key, fd: OwnedFd) -> u64 {
        let id = self.id_for_key(key);
        self.real
            .entry(id)
            .and_modify(|real| real.lookups += 1)
            .or_insert_with(|| Real {
                fd: Arc::new(fd),
                realm: key.0,
                lookups: 1,
            });
        id
    }

    pub fn forget(&mut self, id: u64, lookups: u64) {
        if let Some(real) = self.real.get_mut(&id) {
            real.lookups = real.lookups.saturating_sub(lookups);
            if real.lookups == 0 {
                self.real.remove(&id);
            }
        }
    }

    fn allocate(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn fd() -> OwnedFd {
        File::open("/").unwrap().into()
    }

    #[test]
    fn same_key_gives_same_id() {
        let mut nodes = Nodes::default();
        let first = nodes.remember((0, 1, 10), fd());
        let second = nodes.remember((0, 1, 10), fd());
        assert_eq!(first, second);
        assert_ne!(first, nodes.remember((0, 1, 11), fd()));
    }

    #[test]
    fn id_survives_forget() {
        let mut nodes = Nodes::default();
        let id = nodes.remember((0, 1, 10), fd());
        nodes.forget(id, 1);
        assert!(nodes.get(id).is_none());
        assert_eq!(nodes.remember((0, 1, 10), fd()), id);
        assert!(nodes.get(id).is_some());
    }

    #[test]
    fn descriptor_stays_until_every_lookup_is_forgotten() {
        let mut nodes = Nodes::default();
        let id = nodes.remember((0, 1, 10), fd());
        nodes.remember((0, 1, 10), fd());
        nodes.forget(id, 1);
        assert!(nodes.get(id).is_some());
        nodes.forget(id, 1);
        assert!(nodes.get(id).is_none());
    }

    #[test]
    fn project_ids_are_stable_and_distinct() {
        let mut nodes = Nodes::default();
        let proj1 = nodes.project_id(&"proj1".into());
        assert_eq!(nodes.project_id(&"proj1".into()), proj1);
        assert_ne!(nodes.project_id(&"proj2".into()), proj1);
        assert_ne!(proj1, ROOT_ID);
        assert!(matches!(nodes.get(proj1), Some(Node::Project(name)) if name == "proj1"));
    }
}
