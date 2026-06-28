mod budget;
mod check;
mod components;
mod filter;
mod heap_guard;
mod resources;
mod stack;

use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::AssetEventSystems,
    camera::{
        visibility::{
            add_visibility_class, Visibility, VisibilityClass, VisibilitySystems::CheckVisibility,
        },
        Camera,
    },
    diagnostic::{Diagnostic, DiagnosticPath, RegisterDiagnostic, DEFAULT_MAX_HISTORY_LENGTH},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
    transform::TransformSystems,
};
pub use check::*;
pub use components::*;
pub use filter::*;
pub use resources::*;
use stack::*;

use crate::PointCloud3d;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PointCloudVisibilitySystems {
    CalculateBounds,
    CheckPointCloudNodesVisibility,
}

pub struct PointCloudVisiblityPlugin;

impl PointCloudVisiblityPlugin {
    /// Visibility check diagnostic
    pub const VISIBILITY_CHECK_TIME: DiagnosticPath =
        DiagnosticPath::const_new("pcl_octree_visibility_check");

    /// Budget diagnostic
    pub const BUDGET: DiagnosticPath = DiagnosticPath::const_new("pcl_octree_budget");
}

impl Plugin for PointCloudVisiblityPlugin {
    fn build(&self, app: &mut App) {
        app.register_diagnostic(
            Diagnostic::new(Self::VISIBILITY_CHECK_TIME)
                .with_suffix("ms")
                .with_max_history_length(DEFAULT_MAX_HISTORY_LENGTH)
                .with_smoothing_factor(2.0 / (DEFAULT_MAX_HISTORY_LENGTH as f64 + 1.0)),
        );
        app.register_diagnostic(
            Diagnostic::new(Self::BUDGET)
                .with_suffix("ms")
                .with_max_history_length(DEFAULT_MAX_HISTORY_LENGTH),
        );

        app.register_required_components::<PointCloud3d, Visibility>()
            .register_required_components::<PointCloud3d, VisibilityClass>()
            .register_required_components::<Camera, VisiblePointCloudEntities>()
            .register_required_components::<Camera, PointCloudVisibilitySettings>()
            .init_resource::<GlobalVisiblePointCloudNodes>()
            .add_systems(
                PostUpdate,
                (
                    calculate_bounds.in_set(PointCloudVisibilitySystems::CalculateBounds),
                    check_point_cloud_nodes_visibility
                        .in_set(PointCloudVisibilitySystems::CheckPointCloudNodesVisibility),
                ),
            )
            .configure_sets(
                PostUpdate,
                PointCloudVisibilitySystems::CheckPointCloudNodesVisibility.after(CheckVisibility),
            )
            .configure_sets(
                PostUpdate,
                PointCloudVisibilitySystems::CalculateBounds
                    .before(CheckVisibility)
                    .after(TransformSystems::Propagate)
                    .after(AssetEventSystems),
            );

        app.world_mut()
            .register_component_hooks::<PointCloud3d>()
            .on_add(add_visibility_class::<PointCloud3d>);
    }
}
