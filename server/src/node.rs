use proton_shared::node_def::*;
use proton_shared::node_value::*;
use proton_shared::NODE_DEF_REGISTRY;
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

impl Node {
    pub fn prepare(&self) -> Option<Box<dyn NodeExecutor>> {
        let def = NODE_DEF_REGISTRY.get_def(&self.def_name);
        match &def.runner {
            NodeDefRunner::Executor(ctor) => Some(ctor()),
            _ => None
        }
    }

    pub fn evaluate(
        &self, 
        evaluated_outputs: &HashMap<NodeOutputRef, NodeValue>, 
        executor: Option<Box<dyn NodeExecutor>>
    ) -> Vec<NodeValue> {
        let mut input_vals = Vec::<&NodeValue>::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let input_val = match input {
                NodeInput::Const(val) => val,
                NodeInput::Wire(output_ref) => evaluated_outputs.get(&output_ref).unwrap(),
            };
            input_vals.push(input_val);
        }

        let def = NODE_DEF_REGISTRY.get_def(&self.def_name);
        match &def.runner {
            NodeDefRunner::Function(func) => func(input_vals),
            NodeDefRunner::Executor(_) => executor.unwrap().execute(input_vals),
            NodeDefRunner::OutputDevice(od) => {
                (od.run)(input_vals);
                vec![]
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use proton_shared::node_def::*;
    use proton_shared::node_value::*;
    use proton_shared::NODE_DEF_REGISTRY;
    use super::*;

    #[test]
    fn evaluates_function() {
        NODE_DEF_REGISTRY.reset();
        NODE_DEF_REGISTRY.register(
            "test_def".to_owned(), 
            node_def_from_fn!(|count_1: i64, count_2: i64| -> (i64) {
                return vec![NodeValue::Count(count_1 + count_2)];
            }));

        let node = make_node!{
            1: test_def[
                i64{1},
                Wire{2, 0}
            ]
        };
        let map = map!{super::NodeOutputRef {from_node_id: 2, node_output_index: 0} => NodeValue::Count(2)};
        let result = node.evaluate(&map, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], NodeValue::Count(3));
    }
}