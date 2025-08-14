use crate::render::depth_pass::Depth3d;
use crate::render::depth_pass::texture::{ViewDepthPrepassTextures, ViewDepthTexture};
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

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct DepthPassLabel;

#[derive(Default)]
pub struct CustomDrawNode;
impl ViewNode for CustomDrawNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewTarget,
        Option<&'static ViewDepthTexture>,
        &'static ViewDepthPrepassTextures,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, target, view_depth_texture, view_prepass_textures): QueryItem<
            'w,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(stencil_phases) = world.get_resource::<ViewSortedRenderPhases<Depth3d>>() else {
            return Ok(());
        };

        let Some(view_depth_texture) = view_depth_texture else {
            return Ok(());
        };

        let depth_stencil_attachment = Some(view_depth_texture.get_attachment(StoreOp::Store));

        // Get the view entity from the graph
        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(stencil_phase) = stencil_phases.get(&view.retained_view_entity) else {
            return Ok(());
        };

        render_context.add_command_buffer_generation_task(move |render_device| {
            // Command encoder setup
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("prepass_command_encoder"),
                });

            // Render pass setup
            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("depth pass"),
                // No need for the color output in depth phase
                color_attachments: &[],
                // But we store the depth
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = stencil_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the stencil phase {err:?}");
            }

            drop(render_pass);

            if let Some(prepass_depth_texture) = &view_prepass_textures.depth {
                command_encoder.copy_texture_to_texture(
                    view_depth_texture.texture.as_image_copy(),
                    prepass_depth_texture.texture.texture.as_image_copy(),
                    view_prepass_textures.size,
                );
            }

            command_encoder.finish()
        });

        Ok(())
    }
}
