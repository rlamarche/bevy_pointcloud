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
    /// Check if a node can be added while respecting the budget.
    /// Returns true if the node can be added.
    pub fn check(&self, node: &PointCloudNode) -> bool {
        if self.total_nodes + 1 >= MAX_NODES {
            return false;
        }

        if let Some(max_depth) = self.max_depth
            && node.depth > max_depth
        {
            return false;
        }

        if let Some(point_budget) = self.point_budget
            && self.total_points + node.point_count > point_budget
        {
            return false;
        }

        true
    }

    /// Add a node to the budget. This function does not check the budget.
    pub fn add_node(&mut self, node: &PointCloudNode) -> bool {
        let check = self.check(node);
        self.total_points += node.point_count;
        self.total_nodes += 1;

        check
    }

    pub fn value(&self) -> f64 {
        self.total_points as f64
    }
}
