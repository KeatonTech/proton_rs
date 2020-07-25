use super::node::{Node, NodeInput, NodeOutputRef, NodeInputDiscriminants};
use proton_shared::node_value::*;
use std::collections::{HashMap, HashSet};
use std::iter::Iterator;
use crossbeam_channel::bounded;
use std::cmp::min;
use std::sync::RwLockReadGuard;
use super::threadrunner::Threadrunner;

/// Represents the current state of a ComputeGraph, including any errors that may
/// prevent it from executing.
#[derive(Debug, Clone)]
pub enum ComputeGraphState {
    Unprepared,
    ErrFoundCycle,
    ErrInvalidWire {from_node: u32, to_missing_node: u32},
    Ready
}

type EvaluatedOutputsReader = HashMap<NodeOutputRef, NodeValue>;

/// A ComputeGraph is a set of connected nodes, where each node is a compute operation
/// that can rely on the results of other compute operations as inputs. ComputeGraphs
/// can be automatically parallelized because Nodes cannot have side effects.
pub struct ComputeGraph {
    nodes: HashMap<u32, Node>,
    state: ComputeGraphState,

    /// Waves represent 'waves' of processing, where each Node in a wave relies
    /// only on Nodes in a previous wave. This means Nodes in the same wave can
    /// by definition be executed in parallel. Computed lazily.
    waves: Option<Vec<Vec<u32>>>,

    /// Maximum number of operations that can execute simultaneously. Upper bound
    /// on the useful number of threads.
    runner: Option<Threadrunner<(Node, EvaluatedOutputsReader), Option<Vec<NodeValue>>>>,
}

fn process_node(input: &(Node, EvaluatedOutputsReader)) -> Option<Vec<NodeValue>> {
    let (node, evaluated_outputs) = input;
    node.evaluate(&evaluated_outputs)
}

impl ComputeGraph {

    /// Creates a new ComputeGraph with a collection of Nodes.
    pub fn new(nodes_list: &mut dyn Iterator<Item = Node>) -> ComputeGraph {
        let mut nodes = HashMap::new();
        for node in nodes_list {
            nodes.insert(node.id, node);
        }
        ComputeGraph {
            nodes: nodes,
            state: ComputeGraphState::Unprepared,
            waves: None,
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

        let max_parallel = maybe_max_parallel.unwrap();
        let thread_count = min(max_parallel, max_threads);
        self.runner = Some(Threadrunner::new_with_max_batch_size(
            thread_count as u32, 
            max_parallel as usize, 
            process_node));
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
        let mut nodes_in_any_wave = HashSet::<u32>::with_capacity(self.nodes.len());
        let mut waves = Vec::<Vec<u32>>::new();
        let mut max_parallel = 1;

        while nodes_in_any_wave.len() != self.nodes.len() {
            let mut wave = Vec::<u32>::new();

            'outer: for (node_id, deps) in dep_graph.iter() {
                if nodes_in_any_wave.contains(node_id) {
                    break;
                }

                for dep in deps {
                    if !nodes_in_any_wave.contains(&dep) {
                        break 'outer;
                    }
                }

                nodes_in_any_wave.insert(*node_id);
                wave.push(*node_id);
            }

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
        self.state = ComputeGraphState::Ready;
        return Some(max_parallel);
    }

    /// Build a map of each node and the other nodes it relies on.
    fn build_deps_graph(&self) -> HashMap<u32, Vec<u32>> {
        self.nodes.values().map(|node| (
            node.id, 
            node.inputs.iter()
                .filter(|input| NodeInputDiscriminants::from(*input) == NodeInputDiscriminants::Wire)
                .map(|input| -> u32 {
                    if let NodeInput::Wire(wire) = input {
                        wire.from_node_id
                    } else {
                        panic!();
                    }
                })
                .collect()
        )).collect()
    }

    /// Executes the graph using at most the specified number of threads.
    /// Returns None if execution could not complete.
    pub fn execute(&self, max_thread_count: u16) -> Option<HashMap<NodeOutputRef, NodeValue>> {
        None
    }
}