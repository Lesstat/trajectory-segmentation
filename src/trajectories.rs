use crate::graph::path::{Path, PathSplit};
use crate::graph::{
    dijkstra::{find_path, Dijkstra},
    Graph,
};
use crate::graphml::EdgeLookup;
use crate::helpers::{randomized_preference, MyVec, EQUAL_WEIGHTS};

use serde::{Deserialize, Serialize};
use serde_json::from_reader;

use rand::prelude::ThreadRng;
use std::string::ToString;

#[derive(Debug, Deserialize, Serialize)]
pub struct Trajectory {
    pub trip_id: i64,
    pub vehicle_id: i64,
    pub path: MyVec<i64>,
}

pub fn check_trajectory(tra: &Trajectory, graph: &Graph, edge_lookup: &EdgeLookup) -> bool {
    tra.path.windows(2).all(|window| {
        let e0_idx = edge_lookup
            .get(&window[0].to_string())
            .unwrap_or_else(|| panic!("Could not find edge {}", window[0]));
        let e1_idx = edge_lookup
            .get(&window[1].to_string())
            .unwrap_or_else(|| panic!("Could not find edge {}", window[1]));

        let edge0 = &graph.edges[*e0_idx];
        let edge1 = &graph.edges[*e1_idx];

        edge0.target_id == edge1.source_id
    })
}

pub fn read_trajectories<P: AsRef<std::path::Path>>(
    file_path: P,
) -> Result<Vec<Trajectory>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(file_path)?;
    let file = std::io::BufReader::new(file);

    Ok(from_reader(file)?)
}

impl Trajectory {
    pub fn to_path(&self, graph: &Graph, edge_lookup: &EdgeLookup) -> Path {
        let id = self.trip_id as u32;
        let edges: Vec<u32> = self
            .path
            .iter()
            .map(|id| edge_lookup[&id.to_string()])
            .collect();

        let first_node = edges.iter().take(1).map(|e| &graph.edges[*e].source_id);
        let rest_nodes = edges.iter().map(|e| &graph.edges[*e].target_id);

        let nodes: Vec<u32> = first_node.chain(rest_nodes).copied().collect();

        let algo_split = None;
        let total_dimension_costs = [0.0; super::EDGE_COST_DIMENSION];

        let user_split = PathSplit {
            cuts: MyVec::new(),
            alphas: MyVec::new(),
            dimension_costs: MyVec::new(),
            costs_by_alpha: MyVec::new(),
        };
        let node_count = nodes.len();
        let mut path = Path {
            id,
            nodes: MyVec(nodes),
            edges: MyVec(edges),
            user_split,
            algo_split,
            total_dimension_costs,
        };

        path.total_dimension_costs = path.get_subpath_costs(graph, 0, node_count as u32 - 1);

        path
    }

    pub fn filter_out_self_loops(&mut self, graph: &Graph, edge_lookup: &EdgeLookup) -> MyVec<u32> {
        let (normal, self_loops): (Vec<_>, Vec<_>) =
            self.path.iter().enumerate().partition(|(_, e)| {
                let e_idx = *edge_lookup
                    .get(&e.to_string())
                    .unwrap_or_else(|| panic!("can not find edge {}", e));
                let edge = &graph.edges[e_idx];
                edge.source_id != edge.target_id
            });

        let indices = MyVec(self_loops.into_iter().map(|(i, _)| i as u32).collect());
        self.path = MyVec(normal.into_iter().map(|(_, e)| e).copied().collect());

        indices
    }
}

pub fn create_randomwalk_trajectory(
    source: u32,
    target: u32,
    graph: &Graph,
    d: &mut Dijkstra,
    rng: &mut ThreadRng,
) -> Option<Trajectory> {
    let mut cur_node = source;
    let mut path = MyVec::new();

    let _ = graph.find_shortest_path(d, 0, &[cur_node, target], EQUAL_WEIGHTS)?;
    while cur_node != target {
        let alpha = randomized_preference(rng);
        let tmp_path = graph
            .find_shortest_path(d, 0, &[cur_node, target], alpha)
            .expect("There must be a path");
        find_path(d, &[cur_node, target], alpha)?;

        let first_edge = tmp_path.edges[0 as usize];

        path.push(first_edge as i64);
        cur_node = tmp_path.nodes[1 as usize];
    }

    Some(Trajectory {
        trip_id: -1,
        vehicle_id: -1,
        path,
    })
}
