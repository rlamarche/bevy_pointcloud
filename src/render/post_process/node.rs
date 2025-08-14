use super::PostProcessPipeline;
use crate::render::post_process::texture::PostProcessBindGroup;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_render::view::ViewDepthTexture;
use bevy_render::{
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_resource::*,
    renderer::RenderContext,
    view::ViewTarget,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct PostProcessLabel;

// The post process node used for the render graph
#[derive(Default)]
pub struct PostProcessNode;

// The ViewNode trait is required by the ViewNodeRunner
impl ViewNode for PostProcessNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static PostProcessBindGroup,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, depth, post_process_bind_group): QueryItem<
            Self::ViewQuery,
        >,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let post_process_pipeline = world.resource::<PostProcessPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(pipeline) = pipeline_cache.get_render_pipeline(post_process_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("post_process_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &post_process_bind_group.value, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
