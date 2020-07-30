extern crate strum;
#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate lazy_static;

pub mod node_def;
mod node_def_registry;
pub mod node_value;

lazy_static! {
    pub static ref NODE_DEF_REGISTRY: node_def_registry::NodeDefRegistry =
        { node_def_registry::NodeDefRegistry::new() };
}
