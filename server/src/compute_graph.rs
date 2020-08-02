use super::node::{Node, NodeInput, NodeInputDiscriminants, NodeOutputRef};
use parking_lot::RwLock;
use proton_shared::node_def::NodeExecutor;
use proton_shared::node_def_registry::NodeDefRegistry;
use proton_shared::node_value::*;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::iter::Iterator;

/// Represents the current state of a ComputeGraph, including any errors that may
/// prevent it from executing.
#[derive(Debug, Clone, PartialEq)]
pub enum ComputeGraphState {
    Unprepared,
    ErrFoundCycle,
    ErrInvalidWire {
        from_node: u32,
        to_missing_node: u32,
    },
    Ready,
}

/// A ComputeGraph is a set of connected nodes, where each node is a compute operation
/// that can rely on the results of other compute operations as inputs. ComputeGraphs
/// can be automatically parallelized because Nodes cannot have side effects.
pub struct ComputeGraph {
    nodes: HashMap<u32, Node>,
    registry: NodeDefRegistry,
    state: ComputeGraphState,

    /// Waves represent 'waves' of processing, where each Node in a wave relies
    /// only on Nodes in a previous wave. This means Nodes in the same wave can
    /// by definition be executed in parallel. Computed lazily.
    waves: Option<Vec<Vec<u32>>>,

    /// Stores optional NodeExecutor instances for each Node.
    executors: Option<HashMap<u32, Option<Box<dyn NodeExecutor>>>>,

    /// Multithreaded task runner that takes an array of inputs and produces an
    /// array of outputs based on the provided Node evaluator function.
    runner: Option<ThreadPool>,
}

impl ComputeGraph {
    /// Creates a new ComputeGraph with a collection of Nodes.
    pub fn new(node_def_registry: NodeDefRegistry, nodes_list: Vec<Node>) -> ComputeGraph {
        let mut nodes = HashMap::new();
        for node in nodes_list {
            nodes.insert(node.id, node);
        }
        ComputeGraph {
            nodes: nodes,
            registry: node_def_registry,
            state: ComputeGraphState::Unprepared,
            waves: None,
            executors: None,
            runner: None,
        }
    }

    /// When false, `.prepare` must be run on this graph before it can be executed.
    pub fn get_state(&self) -> ComputeGraphState {
        self.state.clone()
    }

    /// Adds or updates a Node in the graph
    pub fn set_node(&mut self, node: Node) {
        self.nodes.insert(node.id, node);
        self.state = ComputeGraphState::Unprepared;
        self.waves = None;
    }

    /// Removes a Node from the graph
    pub fn remove_node(&mut self, node_id: &u32) {
        self.nodes.remove(node_id);
        self.state = ComputeGraphState::Unprepared;
        self.waves = None;
    }

    /// Prepares the ComputeGraph to be executed by ordering nodes into waves that
    /// can be evaluated in parallel. This is based on a fairly simple topological
    /// sorting algorithm. Can be optimized in the future as necessary.
    ///
    /// Returns false if the input graph is invalid, such as if it contains a cycle.
    pub fn prepare(&mut self, max_threads: u16) -> bool {
        let maybe_max_parallel = self.prepare_graph_order();
        if maybe_max_parallel.is_none() {
            return false;
        }

        // Prepare a threadpool for execution
        let max_parallel = maybe_max_parallel.unwrap();
        let thread_count = min(max_parallel, max_threads);
        self.runner = Some(
            ThreadPoolBuilder::new()
                .num_threads(thread_count.into())
                .build()
                .unwrap(),
        );

        // Prepare each node.
        let active_outputs_per_node = self.compute_active_outputs();
        let nodes = &self.nodes;
        self.executors = self.runner.as_ref().unwrap().install(|| {
            return Some(
                nodes
                    .par_iter()
                    .map(|(id, node)| {
                        (
                            *id,
                            node.with_registry(&self.registry)
                                .prepare(active_outputs_per_node.get(id).unwrap()),
                        )
                    })
                    .collect(),
            );
        });

        self.state = ComputeGraphState::Ready;
        return true;
    }

    /// Topologially sorts the graph into a canonical execution order. Returns the
    /// maximum number of operation that can ever execute in parallel, whih puts an
    /// upper bound on the number of threads to use.
    fn prepare_graph_order(&mut self) -> Option<u16> {
        if self.waves != None {
            return None;
        }

        // Build a map of each node and the other nodes it relies on.
        let dep_graph = self.build_deps_graph();

        // Collect that map into waves.
        let mut nodes_in_prev_wave = HashSet::<u32>::with_capacity(self.nodes.len());
        let mut nodes_in_this_wave = HashSet::<u32>::with_capacity(self.nodes.len());
        let mut waves = Vec::<Vec<u32>>::new();
        let mut max_parallel = 1;

        while nodes_in_prev_wave.len() != self.nodes.len() {
            let mut wave = Vec::<u32>::new();

            'outer: for (node_id, deps) in dep_graph.iter() {
                if nodes_in_prev_wave.contains(node_id) {
                    continue;
                }

                for dep in deps {
                    if !nodes_in_prev_wave.contains(&dep) {
                        continue 'outer;
                    }
                }

                nodes_in_this_wave.insert(*node_id);
                wave.push(*node_id);
            }

            nodes_in_prev_wave.extend(nodes_in_this_wave.iter());
            nodes_in_this_wave.clear();

            if wave.len() == 0 {
                // An empty wave means there's a cycle.
                self.state = ComputeGraphState::ErrFoundCycle;
                return None;
            }
            if wave.len() as u16 > max_parallel {
                max_parallel = wave.len() as u16;
            }
            waves.push(wave);
        }

        self.waves = Some(waves);
        return Some(max_parallel);
    }

    /// Build a map of each node and the other nodes it relies on.
    fn build_deps_graph(&self) -> HashMap<u32, Vec<u32>> {
        self.nodes
            .values()
            .map(|node| {
                (
                    node.id,
                    node.inputs
                        .iter()
                        .filter(|input| {
                            NodeInputDiscriminants::from(*input) == NodeInputDiscriminants::Wire
                        })
                        .map(|input| -> u32 {
                            if let NodeInput::Wire(wire) = input {
                                wire.from_node_id
                            } else {
                                panic!();
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }

    /// Determines which outputs of each Node are actively in use.
    fn compute_active_outputs(&self) -> HashMap<u32, Vec<bool>> {
        let all_wires = self.nodes.values().flat_map(|node| {
            node.inputs
                .iter()
                .filter(|input| {
                    NodeInputDiscriminants::from(*input) == NodeInputDiscriminants::Wire
                })
                .map(|input| -> &NodeOutputRef {
                    if let NodeInput::Wire(wire) = input {
                        wire
                    } else {
                        panic!();
                    }
                })
        });

        let mut result: HashMap<u32, Vec<bool>> = self
            .nodes
            .values()
            .map(|node| {
                (
                    node.id,
                    vec![false; node.with_registry(&self.registry).get_output_count()],
                )
            })
            .collect();

        for wire in all_wires {
            *result
                .get_mut(&wire.from_node_id)
                .unwrap()
                .get_mut(wire.node_output_index as usize)
                .unwrap() = true;
        }

        return result;
    }

    /// Executes the graph using at most the specified number of threads.
    /// Returns None if execution could not complete.
    pub fn execute(&self) -> Result<HashMap<NodeOutputRef, NodeValue>, &str> {
        if self.state != ComputeGraphState::Ready {
            return Err("Must call .prepare() before executing the graph.");
        }
        let executors = &self.executors.as_ref().unwrap();

        let ret = RwLock::new(HashMap::<NodeOutputRef, NodeValue>::new());
        self.runner.as_ref().unwrap().install(|| {
            for wave in self.waves.as_ref().unwrap() {
                let mut results = Vec::<Vec<NodeValue>>::new();
                {
                    let reader = ret.read();
                    wave.par_iter()
                        .map(|node_id: &u32| {
                            self.nodes
                                .get(node_id)
                                .unwrap()
                                .with_registry(&self.registry)
                                .evaluate(&reader, executors.get(&node_id).unwrap())
                        })
                        .collect_into_vec(&mut results);
                }
                let mut writer = ret.write();
                for (i, result) in results.into_iter().enumerate() {
                    let node_id = wave[i];
                    for (j, val) in result.into_iter().enumerate() {
                        writer.insert(
                            NodeOutputRef {
                                from_node_id: node_id as u32,
                                node_output_index: j as u8,
                            },
                            val,
                        );
                    }
                }
            }
        });

        return Ok(ret.into_inner());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::*;
    use proton_shared::node_def::*;
    use proton_shared::node_def_registry::NodeDefRegistry;

    #[test]
    fn executes_simple_graphs() {
        let registry = NodeDefRegistry::new();

        registry.register(
            "output_1".to_owned(),
            node_def_from_fn!(|| -> (i64) {
                return vec![NodeValue::Count(1)];
            }),
        );
        registry.register(
            "add".to_owned(),
            node_def_from_fn!(|count_1: i64, count_2: i64| -> (i64) {
                return vec![NodeValue::Count(count_1 + count_2)];
            }),
        );

        let nodes = make_nodes! {
            1: output_1[],
            2: add[Wire{1, 0}, i64{3}],
            3: add[Wire{1, 0}, i64{5}],
            4: add[Wire{2, 0}, Wire{3, 0}]
        };
        let mut graph = ComputeGraph::new(registry, nodes);

        graph.prepare(2);
        let result = graph.execute().unwrap();
        assert_eq!(
            result
                .get(&NodeOutputRef {
                    from_node_id: 4,
                    node_output_index: 0
                })
                .unwrap(),
            &NodeValue::Count(10)
        );
    }
}
