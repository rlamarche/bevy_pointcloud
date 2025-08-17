use crate::render::normalize_pass::pipeline::NormalizePassPipelineId;
use crate::render::normalize_pass::texture::NormalizePassBindGroup;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_render::view::ViewDepthTexture;
use bevy_render::{
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_resource::*,
    renderer::RenderContext,
    view::ViewTarget,
};
use crate::render::normalize_pass::eye_dome_lighting::NormalizePassEdlBindgroup;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct NormalizePassLabel;

#[derive(Default)]
pub struct NormalizePassNode;

// The ViewNode trait is required by the ViewNodeRunner
impl ViewNode for NormalizePassNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static NormalizePassBindGroup,
        Option<&'static NormalizePassEdlBindgroup>,
        &'static NormalizePassPipelineId,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, depth, textures_bind_group, edl_bind_group, pipeline_id): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else {
            return Ok(());
        };

        let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("pcl_normalize_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &textures_bind_group.value, &[]);

        if let Some(edl_bind_group) = edl_bind_group {
            render_pass.set_bind_group(1, &edl_bind_group.value, &[]);
        }
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
