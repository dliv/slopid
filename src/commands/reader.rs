use crate::documents::{self, CanonicalNode, Edge, EdgeType, Finding};
use anyhow::{Result, bail};
use clap::ValueEnum;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum GraphDirection {
    #[default]
    Both,
    Outgoing,
    Incoming,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum GraphEdgeType {
    Origin,
    Supersedes,
    Related,
}

impl GraphEdgeType {
    fn document_type(self) -> EdgeType {
        match self {
            Self::Origin => EdgeType::Origin,
            Self::Supersedes => EdgeType::Supersedes,
            Self::Related => EdgeType::Related,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ResolveResult {
    pub node: CanonicalNode,
}

#[derive(Debug, Serialize)]
pub struct GraphResult {
    pub anchor: String,
    pub complete: bool,
    pub nodes: Vec<CanonicalNode>,
    pub edges: Vec<Edge>,
    pub findings: Vec<Finding>,
}

pub fn cmd_resolve(id: &str, cwd: &Path) -> Result<ResolveResult> {
    let config = super::load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    let Some(record) = index.nodes.get(id) else {
        bail!("no unique STM entrypoint has canonical id {id}");
    };
    Ok(ResolveResult {
        node: record.node.clone(),
    })
}

pub fn cmd_graph(
    anchor: &str,
    depth: Option<usize>,
    direction: GraphDirection,
    edge_types: &[GraphEdgeType],
    cwd: &Path,
) -> Result<GraphResult> {
    let config = super::load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    if !index.nodes.contains_key(anchor) {
        bail!("no unique STM entrypoint has canonical id {anchor}");
    }

    let selected_types = if edge_types.is_empty() {
        [EdgeType::Origin, EdgeType::Supersedes, EdgeType::Related]
            .into_iter()
            .collect::<BTreeSet<_>>()
    } else {
        edge_types
            .iter()
            .map(|edge_type| edge_type.document_type())
            .collect()
    };
    let selected_edges = index
        .edges
        .iter()
        .filter(|edge| selected_types.contains(&edge.edge_type))
        .collect::<Vec<_>>();

    let mut distances = HashMap::from([(anchor.to_string(), 0_usize)]);
    let mut queue = VecDeque::from([anchor.to_string()]);
    while let Some(current) = queue.pop_front() {
        let current_depth = distances[&current];
        if depth.is_some_and(|limit| current_depth >= limit) {
            continue;
        }
        for edge in &selected_edges {
            let neighbor = if edge.edge_type == EdgeType::Related {
                if edge.source == current {
                    Some(&edge.target)
                } else if edge.target == current {
                    Some(&edge.source)
                } else {
                    None
                }
            } else {
                match direction {
                    GraphDirection::Both if edge.source == current => Some(&edge.target),
                    GraphDirection::Both if edge.target == current => Some(&edge.source),
                    GraphDirection::Outgoing if edge.source == current => Some(&edge.target),
                    GraphDirection::Incoming if edge.target == current => Some(&edge.source),
                    _ => None,
                }
            };
            if let Some(neighbor) = neighbor
                && !distances.contains_key(neighbor)
            {
                distances.insert(neighbor.clone(), current_depth + 1);
                queue.push_back(neighbor.clone());
            }
        }
    }

    let selected_ids = distances.keys().cloned().collect::<BTreeSet<_>>();
    let mut records = selected_ids
        .iter()
        .filter_map(|id| index.nodes.get(id).map(|record| (id, record)))
        .collect::<Vec<_>>();
    records.sort_by(|(left_id, left), (right_id, right)| {
        (&left.sort_key, left_id, &left.node.path).cmp(&(
            &right.sort_key,
            right_id,
            &right.node.path,
        ))
    });
    let nodes = records
        .into_iter()
        .map(|(_, record)| record.node.clone())
        .collect();
    let edges = selected_edges
        .into_iter()
        .filter(|edge| selected_ids.contains(&edge.source) && selected_ids.contains(&edge.target))
        .cloned()
        .collect();

    Ok(GraphResult {
        anchor: anchor.to_string(),
        complete: index.complete(),
        nodes,
        edges,
        findings: index.graph_findings(),
    })
}
