use super::node_def::NodeDef;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use std::collections::HashMap;

pub struct NodeDefRegistry {
    map: RwLock<HashMap<String, NodeDef>>,
}

impl NodeDefRegistry {
    pub fn new() -> NodeDefRegistry {
        NodeDefRegistry {
            map: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, node_def_name: String, node_def: NodeDef) {
        if self.map.read().contains_key(&node_def_name) {
            panic!(node_def_name + " already registered as a node def");
        }
        self.map.write().insert(node_def_name, node_def);
    }

    pub fn get_def(&self, node_def_name: &String) -> MappedRwLockReadGuard<NodeDef> {
        RwLockReadGuard::map(self.map.read(), |hashmap| {
            hashmap.get(node_def_name).unwrap_or_else(|| {
                panic!("No such node type: ".to_owned() + node_def_name);
            })
        })
    }

    pub fn reset(&self) {
        self.map.write().clear()
    }
}
