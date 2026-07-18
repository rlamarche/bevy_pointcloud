mod budget;
mod check;
mod components;
mod filter;
mod heap_guard;
mod light;
mod resources;
mod stack;

use bevy::{
    app::{App, Plugin, PostUpdate},
    camera::{
        visibility::{add_visibility_class, Visibility, VisibilityClass, VisibilitySystems},
        Camera,
    },
    diagnostic::{Diagnostic, DiagnosticPath, RegisterDiagnostic, DEFAULT_MAX_HISTORY_LENGTH},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
    light::{
        check_dir_light_mesh_visibility, DirectionalLight,
        SimulationLightSystems::CheckLightVisibility,
    },
    transform::TransformSystems,
};
pub use check::*;
pub use components::*;
pub use filter::*;
pub use light::*;
pub use resources::*;
use stack::*;

use crate::{PointCloud3d, PointCloudChunk3d, PointCloudServerSystems};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum PointCloudVisibilitySystems {
    CalculateBounds,
    CheckPointCloudNodesVisibility,
    UpdateViewVisibility,
    CheckLightVisibility,
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
            .register_required_components::<PointCloudChunk3d, Visibility>()
            .register_required_components::<PointCloudChunk3d, VisibilityClass>()
            .register_required_components::<Camera, VisiblePointCloudEntities>()
            .register_required_components::<Camera, PointCloudVisibilitySettings>()
            .register_required_components::<DirectionalLight, CascadesVisiblePointCloudEntities>()
            .register_required_components::<DirectionalLight, PointCloudVisibilitySettings>()
            .init_resource::<GlobalVisiblePointCloudNodes>()
            .init_resource::<GlobalVisiblePointCloudChunks>()
            .add_systems(
                PostUpdate,
                (
                    check_point_cloud_nodes_visibility
                        .in_set(PointCloudVisibilitySystems::CheckPointCloudNodesVisibility),
                    set_visible_point_cloud_chunk_visibility
                        .in_set(PointCloudVisibilitySystems::UpdateViewVisibility),
                    check_point_cloud_nodes_dir_lights_visibility
                        .in_set(PointCloudVisibilitySystems::CheckLightVisibility)
                        .after(check_dir_light_mesh_visibility),
                ),
            )
            .configure_sets(
                PostUpdate,
                (
                    PointCloudVisibilitySystems::CheckPointCloudNodesVisibility,
                    // scheduled after [`PointCloudServerSystems`] to have the latest loaded meshes
                    // available for rendering
                    PointCloudVisibilitySystems::UpdateViewVisibility
                        .after(PointCloudServerSystems)
                        // before check light visibility so the shadows can work
                        .before(CheckLightVisibility),
                )
                    .chain()
                    .after(VisibilitySystems::CheckVisibility)
                    // We need the [`GlobalTransform`] to be available when computing visibility.
                    // Note that [`VisibilitySystems::CheckVisibility`] is already after
                    // [`TransformSystems::Propagate`], but we keep this dependency for
                    // information.s
                    .after(TransformSystems::Propagate),
            );

        app.world_mut()
            .register_component_hooks::<PointCloud3d>()
            .on_add(add_visibility_class::<PointCloud3d>);
    }
}
