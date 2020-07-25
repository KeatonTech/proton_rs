use proton_shared::node_def::*;
use proton_shared::node_value::*;
use proton_shared::NODE_DEF_REGISTRY;
use std::collections::HashMap;
use std::sync::Arc;

/// Instance of an executable function as represented in a compute graph.
/// Each Node has a type (a NodeDef) that defines what inputs to take, what outputs
/// to provide, and how to execute. Each Node instance can attach to other Nodes to
/// drive its inputs and outputs. Nodes are composed into a directed acyclic
/// ComputeGraph that can then be evaluated in parallel.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    def_name: String,

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
    pub fn evaluate(&self, evaluated_outputs: &HashMap<NodeOutputRef, NodeValue>) -> Option<Vec<NodeValue>> {
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
            NodeDefRunner::Function(func) => Some(func(input_vals)),
            NodeDefRunner::OutputDevice(od) => {
                (od.run)(input_vals);
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use proton_shared::node_def::*;
    use proton_shared::node_value::*;
    use std::sync::Arc;
    use proton_shared::NODE_DEF_REGISTRY;

    macro_rules! map(
        { $($key:expr => $value:expr),+ } => {
            {
                let mut m = ::std::collections::HashMap::new();
                $(
                    m.insert($key, $value);
                )+
                m
            }
         };
    );

    #[test]
    fn evaluates_function() {
        NODE_DEF_REGISTRY.register("test_def".to_owned(), NodeDef {
            desc: NodeDefBasicDescription {
                name: "Add".to_string(),
                description: "Adds two values together".to_string(),
            },
            inputs: vec![
                NodeInputDef {
                    desc: NodeDefBasicDescription {
                        name: "input 1".to_string(),
                        description: "input 1".to_string()
                    },
                    allowed_types: vec![NodeValueType::Count],
                    required: true,
                },
                NodeInputDef {
                    desc: NodeDefBasicDescription {
                        name: "input 2".to_string(),
                        description: "input 2".to_string()
                    },
                    allowed_types: vec![NodeValueType::Count],
                    required: true,
                }
            ],
            output: vec![
                NodeOutputDef {
                    desc: NodeDefBasicDescription {
                        name: "Sum".to_string(),
                        description: "The sum of the two input value".to_string()
                    },
                    output_type: NodeValueType::Count,
                }
            ],
            runner: NodeDefRunner::Function(|inputs: Vec<&NodeValue>| {
                if let NodeValue::Count(count_1) = inputs[0] {
                    if let NodeValue::Count(count_2) = inputs[1] {
                        return vec![NodeValue::Count(count_1 + count_2)];
                    }
                }
                panic!("Incorrect input types");
            }),
        });
        let node = super::Node {
            id: 1,
            def_name: "test_def".to_owned(),
            inputs: vec![
                super::NodeInput::Const(NodeValue::Count(1)),
                super::NodeInput::Wire(super::NodeOutputRef {
                    from_node_id: 2,
                    node_output_index: 0
                })
            ]
        };
        let map = map!{super::NodeOutputRef {from_node_id: 2, node_output_index: 0} => NodeValue::Count(2)};
        let result = node.evaluate(&map).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], NodeValue::Count(3));
    }
}