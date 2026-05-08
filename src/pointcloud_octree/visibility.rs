use crate::{
    octree::{node::OctreeNode, visibility::budget::OctreeNodesBudget},
    pointcloud_octree::{asset::data::PointCloudNodeData, render::prepare::MAX_NODES},
};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

pub struct PointCloudOctreePointBudget {
    pub point_budget: usize,
    pub total_points: usize,
    pub total_nodes: usize,
}

#[derive(Clone, Debug, Reflect, Component)]
pub struct PointCloudOctreeBudgetSettings {
    pub point_budget: usize,
}

impl OctreeNodesBudget<PointCloudNodeData> for PointCloudOctreePointBudget {
    type Settings = usize;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            point_budget: *settings,
            total_points: 0,
            total_nodes: 0,
        }
    }

    fn add_node(&mut self, node: &OctreeNode<PointCloudNodeData>) -> bool {
        if self.total_nodes >= MAX_NODES {
            return false;
        }

        if self.total_points > self.point_budget {
            return false;
        }

        let Some(data) = &node.data else {
            return false;
        };

        self.total_points += data.num_points;
        self.total_nodes += 1;

        true
    }

    fn value(&self) -> f64 {
        self.total_points as f64
    }
}
