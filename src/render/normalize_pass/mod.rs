mod eye_dome_lighting;
mod node;
mod pipeline;
mod texture;

use crate::render::attribute_pass::node::AttributePassLabel;
use crate::render::eye_dome_lighting::EyeDomeLightingUniformBindgroupLayout;
use crate::render::normalize_pass::pipeline::{NormalizePassPipelineId, NormalizePassPipelineKey};
use crate::render::normalize_pass::texture::prepare_normalize_pass_bind_groups;
use crate::render::{PointCloudRenderMode, PointCloudRenderModeOpt};
use bevy_app::prelude::*;
use bevy_core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy_ecs::prelude::*;
use bevy_render::render_resource::{
    PipelineCache, SpecializedRenderPipelines,
};
use bevy_render::view::Msaa;
use bevy_render::{
    render_graph::{RenderGraphApp, ViewNodeRunner}, Render, RenderApp,
    RenderSet,
};
use node::{NormalizePassLabel, NormalizePassNode};
use pipeline::NormalizePassPipeline;
use crate::render::normalize_pass::eye_dome_lighting::prepare_normalize_pass_edl_bind_groups;

pub struct NormalizePassPlugin;

impl Plugin for NormalizePassPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<SpecializedRenderPipelines<NormalizePassPipeline>>()
            .add_render_graph_node::<ViewNodeRunner<NormalizePassNode>>(
                // Specify the label of the graph, in this case we want the graph for 3d
                Core3d,
                // It also needs the label of the node
                NormalizePassLabel,
            )
            .add_systems(
                Render,
                (
                    prepare_normalize_pass_pipelines.in_set(RenderSet::Prepare),
                    prepare_normalize_pass_bind_groups.in_set(RenderSet::PrepareBindGroups),
                    prepare_normalize_pass_edl_bind_groups.in_set(RenderSet::PrepareBindGroups),
                ),
            )
            .add_render_graph_edges(
                Core3d,
                // Specify the node ordering.
                (
                    AttributePassLabel,
                    NormalizePassLabel,
                    Node3d::MainTransparentPass,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Init this resource here because I need it here
            .init_resource::<EyeDomeLightingUniformBindgroupLayout>()
            // Initialize the pipeline
            .init_resource::<NormalizePassPipeline>();
    }
}

fn prepare_normalize_pass_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<NormalizePassPipeline>>,
    pipeline: Res<NormalizePassPipeline>,
    views: Query<(Entity, &Msaa, Option<&PointCloudRenderMode>)>,
) {
    for (entity, msaa, point_cloud_render_mode) in &views {
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            NormalizePassPipelineKey {
                samples: msaa.samples(),
                use_edl: point_cloud_render_mode.use_edl(),
                edl_neighbour_count: point_cloud_render_mode.edl_neighbour_count(),
            },
        );

        commands
            .entity(entity)
            .insert(NormalizePassPipelineId(pipeline_id));
    }
}
