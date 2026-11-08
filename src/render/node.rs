#[cfg(feature = "trace")]
use bevy::log::info_span;
use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    ecs::{system::Res, world::World},
    log::error,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        view::{ExtractedView, ViewDepthTexture, ViewTarget},
    },
};

use crate::render::{render_phase::ViewBinnedChunksRenderPhases, AlphaMask3dChunk, Opaque3dChunk};

pub fn main_opaque_pass_3d_chunk(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthTexture,
        Option<&MainPassResolutionOverride>,
    )>,
    opaque_phases: Res<ViewBinnedChunksRenderPhases<Opaque3dChunk>>,
    alpha_mask_phases: Res<ViewBinnedChunksRenderPhases<AlphaMask3dChunk>>,
    mut ctx: RenderContext,
) {
    let view_entity = view.entity();

    let (camera, extracted_view, target, depth, resolution_override) = view.into_inner();

    let (Some(opaque_phase), Some(alpha_mask_phase)) = (
        opaque_phases.get(&extracted_view.retained_view_entity),
        alpha_mask_phases.get(&extracted_view.retained_view_entity),
    ) else {
        return;
    };

    #[cfg(feature = "trace")]
    let _main_opaque_pass_3d_span = info_span!("main_opaque_pass_3d_chunk").entered();

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();

    let color_attachments = [Some(target.get_color_attachment())];
    let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("main_opaque_pass_3d_chunk"),
        color_attachments: &color_attachments,
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let pass_span = diagnostics.pass_span(&mut render_pass, "main_opaque_pass_3d_chunk");

    if let Some(viewport) =
        Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
    {
        render_pass.set_camera_viewport(&viewport);
    }

    if !opaque_phase.is_empty() {
        #[cfg(feature = "trace")]
        let _opaque_main_pass_3d_span = info_span!("opaque_main_pass_3d_chunk").entered();
        if let Err(err) = opaque_phase.render(&mut render_pass, world, view_entity) {
            error!("Error encountered while rendering the opaque chunk phase {err:?}");
        }
    }

    if !alpha_mask_phase.is_empty() {
        #[cfg(feature = "trace")]
        let _alpha_mask_main_pass_3d_span = info_span!("alpha_mask_main_pass_3d_chunk").entered();
        if let Err(err) = alpha_mask_phase.render(&mut render_pass, world, view_entity) {
            error!("Error encountered while rendering the alpha mask chunk phase {err:?}");
        }
    }

    pass_span.end(&mut render_pass);
}
