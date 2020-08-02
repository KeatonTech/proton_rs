use super::node_value::{NodeValue, NodeValueType};
use std::fmt;

/// A NodeDef represents a type of function that can be called in an evaluation graph.
/// These functions, like Rust's own functions, have a name and defined input and output
/// types (note that NodeDefs can explicitly have multiple outputs). Unlike Rust functions,
/// NodeDefs must execute without causing any side effects, ever. The type system is also
/// unique to Proton in order to better fit the domain and to keep the door open for
/// non-Rust plugins in the future.
#[derive(Debug, PartialEq)]
pub struct NodeDef {
    pub desc: NodeDefBasicDescription,
    pub inputs: Vec<NodeInputDef>,
    pub outputs: Vec<NodeOutputDef>,
    pub runner: NodeDefRunner,
}

/// Represents a single input to a NodeDef function.
#[derive(Debug, PartialEq)]
pub struct NodeInputDef {
    pub desc: NodeDefBasicDescription,
    pub allowed_types: Vec<NodeValueType>,
    pub required: bool,
}

/// Represents a single output of a NodeDef function.
#[derive(Debug, PartialEq)]
pub struct NodeOutputDef {
    pub desc: NodeDefBasicDescription,
    pub output_type: NodeValueType,
}

/// Human-readable information about a node or its inputs or outputs.
#[derive(Debug, PartialEq)]
pub struct NodeDefBasicDescription {
    pub name: String,
    pub description: String,
}

/// Options for executing a Node, as specified in a NodeDef.
pub enum NodeDefRunner {
    Function(fn(Vec<&NodeValue>) -> Vec<NodeValue>),
    Executor(fn() -> Box<dyn NodeExecutor>),
    OutputDevice(NodeDefOutputRunner),
}

pub struct NodeDefOutputRunner {
    pub run: fn(Vec<&NodeValue>),
    pub device: OutputDevice,
}

/// Information about an output device
#[derive(Debug, PartialEq)]
pub struct OutputDevice {
    pub name: String,
}

impl fmt::Debug for NodeDefRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[Node Runner]")
    }
}

impl std::cmp::PartialEq for NodeDefRunner {
    fn eq(&self, _: &Self) -> bool {
        // TODO (see if there's a way to actually make this work)
        true
    }
}

pub trait NodeExecutor: Send + Sync {
    fn prepare(&self, enabled_outputs: &Vec<bool>);
    fn execute(&self, inputs: Vec<&NodeValue>) -> Vec<NodeValue>;
}
