//! The checks that make a spec well-formed (ADR 0022, "Spec"): unique ids,
//! inputs that exist and fit the node type, no cycle, exactly one
//! visualization at the end, and the limits of a graph. Only a well-formed
//! spec can be saved.

use std::collections::HashMap;

use super::attributes;
use super::problem::Problem;
use super::spec::{Spec, is_identifier};
use super::{MAX_NODES, MAX_SOURCES};

/// The graph of a well-formed spec.
#[derive(Debug, Clone)]
pub struct Graph {
    index: HashMap<String, usize>,
    /// Inputs of each node by index, with the names of a match's inputs;
    /// `a` first.
    inputs: Vec<Vec<(Option<String>, usize)>>,
    /// Nodes in an order in which every node comes after its inputs.
    order: Vec<usize>,
    visualization: usize,
}

impl Graph {
    /// Checks that the spec is well-formed and orders its nodes.
    pub fn new(spec: &Spec) -> Result<Graph, Vec<Problem>> {
        let mut problems = Vec::new();
        if spec.nodes.len() > MAX_NODES {
            problems.push(Problem::spec_setting(
                "nodes",
                format!(
                    "a chart has at most {MAX_NODES} nodes; this one has {}",
                    spec.nodes.len()
                ),
            ));
        }
        let sources = spec.nodes.iter().filter(|n| n.kind.is_source()).count();
        if sources > MAX_SOURCES {
            problems.push(Problem::spec_setting(
                "nodes",
                format!("a chart has at most {MAX_SOURCES} sources; this one has {sources}"),
            ));
        }

        let mut index = HashMap::new();
        for (i, node) in spec.nodes.iter().enumerate() {
            if index.insert(node.id.clone(), i).is_some() {
                problems.push(Problem::setting(
                    &node.id,
                    "id",
                    format!("another node has the id {:?}", node.id),
                ));
            }
        }

        let mut inputs: Vec<Vec<(Option<String>, usize)>> = vec![Vec::new(); spec.nodes.len()];
        for (i, node) in spec.nodes.iter().enumerate() {
            let id = &node.id;
            let ty = node.kind.type_name();
            if node.kind.is_source() {
                if node.input.is_some() || node.inputs.is_some() {
                    problems.push(Problem::setting(id, "input", "a source has no input"));
                }
                continue;
            }
            if node.kind.is_match() {
                if node.input.is_some() {
                    problems.push(Problem::setting(
                        id,
                        "input",
                        "a match has named inputs: use inputs, e.g. {\"a\": \"runs\", \"b\": \"reference\"}",
                    ));
                }
                match &node.inputs {
                    None => problems.push(Problem::setting(
                        id,
                        "inputs",
                        "a match needs named inputs, e.g. {\"a\": \"runs\", \"b\": \"reference\"}",
                    )),
                    Some(named) => {
                        if !named.contains_key("a") {
                            problems.push(Problem::setting(
                                id,
                                "inputs",
                                "a match needs the input a: its frames are matched with the others",
                            ));
                        }
                        if named.len() < 2 {
                            problems.push(Problem::setting(
                                id,
                                "inputs",
                                "a match needs at least two inputs",
                            ));
                        }
                        for name in named.keys() {
                            let message = if !is_identifier(name) {
                                format!(
                                    "{name:?} is not an input name: use letters, digits and _, \
                                     starting with a letter"
                                )
                            } else if attributes::is_prefix(name) {
                                format!(
                                    "{name:?} cannot name an input: attributes start with it \
                                     ({name}.…)"
                                )
                            } else {
                                continue;
                            };
                            problems.push(Problem::setting(id, format!("inputs.{name}"), message));
                        }
                    }
                }
            } else {
                if node.inputs.is_some() {
                    problems.push(Problem::setting(
                        id,
                        "inputs",
                        format!("only a match has named inputs; a {ty} has one input"),
                    ));
                }
                if node.input.is_none() {
                    problems.push(Problem::setting(
                        id,
                        "input",
                        format!("a {ty} needs an input"),
                    ));
                }
            }
            for (name, input) in node.input_ids() {
                let setting = match name {
                    Some(name) => format!("inputs.{name}"),
                    None => "input".to_owned(),
                };
                match index.get(input) {
                    None => problems.push(Problem::setting(
                        id,
                        setting,
                        format!("there is no node {input:?}"),
                    )),
                    Some(&j) if spec.nodes[j].kind.is_visualization() => {
                        problems.push(Problem::setting(
                            id,
                            setting,
                            format!(
                                "{input:?} is a visualization, the end of the graph; no node can \
                                 read it"
                            ),
                        ));
                    }
                    Some(&j) => inputs[i].push((name.map(str::to_owned), j)),
                }
            }
        }

        let visualizations: Vec<usize> = spec
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind.is_visualization())
            .map(|(i, _)| i)
            .collect();
        match visualizations.as_slice() {
            [] => problems.push(Problem::spec_setting(
                "nodes",
                "a chart needs a visualization: a line, scatter, bar, box, heatmap, value or table",
            )),
            [_] => {}
            [first, rest @ ..] => {
                for &i in rest {
                    problems.push(Problem::node(
                        &spec.nodes[i].id,
                        format!(
                            "a chart has exactly one visualization; {:?} is the other one",
                            spec.nodes[*first].id
                        ),
                    ));
                }
            }
        }

        let order = match topological_order(&inputs) {
            Ok(order) => order,
            Err(cycle) => {
                let names: Vec<&str> = cycle.iter().map(|&i| spec.nodes[i].id.as_str()).collect();
                problems.push(Problem::node(
                    names[0],
                    format!("the inputs form a cycle: {}", names.join(" → ")),
                ));
                Vec::new()
            }
        };

        if problems.is_empty() {
            Ok(Graph {
                index,
                inputs,
                order,
                visualization: visualizations[0],
            })
        } else {
            Err(problems)
        }
    }

    /// Nodes by index, every node after its inputs.
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// The inputs of node `i`, with the names of a match's inputs; `a`
    /// first.
    pub fn inputs(&self, i: usize) -> &[(Option<String>, usize)] {
        &self.inputs[i]
    }

    /// The index of node `id`.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// The index of the visualization.
    pub fn visualization(&self) -> usize {
        self.visualization
    }

    /// The nodes the node `i` needs, itself included, in evaluation order.
    /// A run evaluates these for the visualization, or for the node a
    /// preview stops at.
    pub fn upstream(&self, i: usize) -> Vec<usize> {
        let mut needed = vec![false; self.inputs.len()];
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            if !needed[n] {
                needed[n] = true;
                stack.extend(self.inputs[n].iter().map(|(_, j)| *j));
            }
        }
        self.order.iter().copied().filter(|&n| needed[n]).collect()
    }
}

/// Kahn's algorithm. On a cycle, returns the nodes of one cycle, closed
/// (the first node again at the end).
fn topological_order(inputs: &[Vec<(Option<String>, usize)>]) -> Result<Vec<usize>, Vec<usize>> {
    let n = inputs.len();
    let mut consumers = vec![Vec::new(); n];
    let mut pending = vec![0usize; n];
    for (i, ins) in inputs.iter().enumerate() {
        for (_, j) in ins {
            consumers[*j].push(i);
            pending[i] += 1;
        }
    }
    let mut ready: Vec<usize> = (0..n).rev().filter(|&i| pending[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = ready.pop() {
        order.push(i);
        for &c in consumers[i].iter().rev() {
            pending[c] -= 1;
            if pending[c] == 0 {
                ready.push(c);
            }
        }
    }
    if order.len() == n {
        return Ok(order);
    }
    // Every node left has an input that is left too: follow inputs until
    // one repeats.
    let start = (0..n).find(|&i| pending[i] > 0).expect("a node is left");
    let mut path = vec![start];
    let mut at = start;
    loop {
        at = inputs[at]
            .iter()
            .map(|(_, j)| *j)
            .find(|&j| pending[j] > 0)
            .expect("a node on a cycle has an input on it");
        if let Some(pos) = path.iter().position(|&p| p == at) {
            let mut cycle: Vec<usize> = path[pos..].to_vec();
            cycle.reverse();
            cycle.push(cycle[0]);
            return Err(cycle);
        }
        path.push(at);
    }
}
