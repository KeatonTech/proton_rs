use proton_shared::node_def::*;
use proton_shared::node_def_registry::NodeDefRegistry;
use proton_shared::node_value::*;
use std::collections::HashMap;

/// Instance of an executable function as represented in a compute graph.
/// Each Node has a type (a NodeDef) that defines what inputs to take, what outputs
/// to provide, and how to execute. Each Node instance can attach to other Nodes to
/// drive its inputs and outputs. Nodes are composed into a directed acyclic
/// ComputeGraph that can then be evaluated in parallel.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub def_name: String,

    /// Order of inputs must match order in NodeDef.
    pub inputs: Vec<NodeInput>,
}

#[derive(Debug, EnumDiscriminants, PartialEq, Clone)]
pub enum NodeInput {
    Const(NodeValue),
    Wire(NodeOutputRef),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct NodeOutputRef {
    pub from_node_id: u32,
    pub node_output_index: u8,
}

pub struct NodeWithRegistry<'a> {
    node: &'a Node,
    registry: &'a NodeDefRegistry,
}

impl Node {
    /// Most Node functionality can only be processed using a NodeDefRegistry,
    /// which gives the Node access to the underlying implementation of its
    /// node_def. This method allows the registry to be passed in once instead
    /// of requiring it as an arg for every single function.
    pub fn with_registry<'a>(&'a self, registry: &'a NodeDefRegistry) -> NodeWithRegistry<'a> {
        NodeWithRegistry {
            node: self,
            registry: registry,
        }
    }
}

impl<'a> NodeWithRegistry<'a> {
    pub fn get_input_count(&self) -> usize {
        self.registry.get_def(&self.node.def_name).inputs.len()
    }

    pub fn get_output_count(&self) -> usize {
        self.registry.get_def(&self.node.def_name).outputs.len()
    }

    pub fn prepare(&self, enabled_outputs: &Vec<bool>) -> Option<Box<dyn NodeExecutor>> {
        let def = self.registry.get_def(&self.node.def_name);
        let maybe_executor = match &def.runner {
            NodeDefRunner::Executor(ctor) => Some(ctor()),
            _ => None,
        };
        if !maybe_executor.is_none() {
            maybe_executor.as_ref().unwrap().prepare(enabled_outputs);
        };
        return maybe_executor;
    }

    pub fn evaluate(
        &self,
        evaluated_outputs: &HashMap<NodeOutputRef, NodeValue>,
        executor: &Option<Box<dyn NodeExecutor>>,
    ) -> Vec<NodeValue> {
        let mut input_vals = Vec::<&NodeValue>::with_capacity(self.node.inputs.len());
        for input in &self.node.inputs {
            let input_val = match input {
                NodeInput::Const(val) => val,
                NodeInput::Wire(output_ref) => evaluated_outputs.get(&output_ref).unwrap(),
            };
            input_vals.push(input_val);
        }

        let def = self.registry.get_def(&self.node.def_name);
        match &def.runner {
            NodeDefRunner::Function(func) => func(input_vals),
            NodeDefRunner::Executor(_) => executor.as_ref().unwrap().execute(input_vals),
            NodeDefRunner::OutputDevice(od) => {
                (od.run)(input_vals);
                vec![]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proton_shared::node_def_registry::NodeDefRegistry;

    #[test]
    fn evaluates_function() {
        let registry = NodeDefRegistry::new();
        registry.register(
            "test_def".to_owned(),
            node_def_from_fn!(|count_1: i64, count_2: i64| -> (i64) {
                return vec![NodeValue::Count(count_1 + count_2)];
            }),
        );

        let node = make_node! {
            1: test_def[
                i64{1},
                Wire{2, 0}
            ]
        };
        let map = map! {super::NodeOutputRef {from_node_id: 2, node_output_index: 0} => NodeValue::Count(2)};
        let result = node.with_registry(&registry).evaluate(&map, &None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], NodeValue::Count(3));
    }
}
