use super::AttributePass;
use super::texture::ViewAttributePrepassTextures;
use crate::render::depth_pass::texture::ViewDepthTexture;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_log::error;
use bevy_render::render_phase::TrackedRenderPass;
use bevy_render::render_resource::{CommandEncoderDescriptor, StoreOp};
use bevy_render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_phase::ViewSortedRenderPhases,
    render_resource::RenderPassDescriptor,
    renderer::RenderContext,
    view::{ExtractedView, ViewTarget},
};

// Render label used to order our render graph node that will render our phase
#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct AttributePassLabel;

#[derive(Default)]
pub struct AttributePassNode;
impl ViewNode for AttributePassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static ViewAttributePrepassTextures,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, target, depth, view_prepass_textures): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(stencil_phases) = world.get_resource::<ViewSortedRenderPhases<AttributePass>>()
        else {
            return Ok(());
        };

        let color_attachments = vec![
            view_prepass_textures
                .attribute
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        // Get the view entity from the graph
        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(stencil_phase) = stencil_phases.get(&view.retained_view_entity) else {
            return Ok(());
        };

        let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

        render_context.add_command_buffer_generation_task(move |render_device| {
            // Command encoder setup
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("prepass_command_encoder"),
                });

            // Render pass setup
            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl attribute pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            // Render the phase
            // This will execute each draw functions of each phase items queued in this phase
            if let Err(err) = stencil_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the stencil phase {err:?}");
            }

            drop(render_pass);

            command_encoder.finish()
        });

        Ok(())
    }
}
