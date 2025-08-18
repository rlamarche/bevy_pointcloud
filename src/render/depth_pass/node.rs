use crate::render::depth_pass::DepthPass3d;
use crate::render::depth_pass::texture::ViewDepthPrepassTextures;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_log::error;
use bevy_render::render_phase::TrackedRenderPass;
use bevy_render::render_resource::{CommandEncoderDescriptor, StoreOp};
use bevy_render::view::ViewDepthTexture;
use bevy_render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_phase::ViewSortedRenderPhases,
    render_resource::RenderPassDescriptor,
    renderer::RenderContext,
    view::ExtractedView,
};

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct DepthPassLabel;

#[derive(Default)]
pub struct DepthPassNode;
impl ViewNode for DepthPassNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        Option<&'static ViewDepthTexture>,
        &'static ViewDepthPrepassTextures,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, view_depth_texture, view_prepass_textures): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(stencil_phases) = world.get_resource::<ViewSortedRenderPhases<DepthPass3d>>()
        else {
            return Ok(());
        };

        let Some(view_depth_texture) = view_depth_texture else {
            return Ok(());
        };

        let color_attachments = vec![
            view_prepass_textures
                .depth
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        let depth_stencil_attachment = Some(view_depth_texture.get_attachment(StoreOp::Store));

        // Get the view entity from the graph
        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(attribute_phase) = stencil_phases.get(&view.retained_view_entity) else {
            return Ok(());
        };

        render_context.add_command_buffer_generation_task(move |render_device| {
            // Command encoder setup
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("pcl_depth_pass_command_encoder"),
                });

            // Render pass setup
            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_depth_pass"),
                // In the depth phase, we write a depth mask to reuse it in normalize pass
                // We could use the depth buffer, but with WebGL we can't bind it in a shader
                color_attachments: &color_attachments,
                // We store the depth for usage in attribute pass
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = attribute_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud depth phase {err:?}");
            }

            drop(render_pass);

            command_encoder.finish()
        });

        Ok(())
    }
}
