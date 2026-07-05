use crate::PointCloudNode;

// TODO define elsewhere
pub const MAX_NODES: usize = 2048;

pub struct PointCloudPointBudget {
    pub point_budget: Option<usize>,
    pub max_depth: Option<u32>,
    pub total_points: usize,
    pub total_nodes: usize,
}

impl PointCloudPointBudget {
    pub fn add_node(&mut self, node: &PointCloudNode) -> bool {
        if node.chunk.is_none() {
            return false;
        }

        if self.total_nodes >= MAX_NODES {
            return false;
        }

        if let Some(max_depth) = self.max_depth
            && node.depth > max_depth
        {
            return false;
        }

        if let Some(point_budget) = self.point_budget
            && self.total_points > point_budget
        {
            return false;
        }

        self.total_points += node.point_count;
        self.total_nodes += 1;

        true
    }

    pub fn value(&self) -> f64 {
        self.total_points as f64
    }
}
