use crate::render::attribute_pass::pipeline::AttributePassPipeline;
use crate::render::depth_pass::texture::ViewDepthPrepassTextures;
use bevy_color::LinearRgba;
use bevy_core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transmissive3d, Transparent3d};
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::SystemParamItem;
use bevy_log::warn;
use bevy_platform::collections::HashMap;
use bevy_render::camera::ExtractedCamera;
use bevy_render::prelude::*;
use bevy_render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass, ViewBinnedRenderPhases,
    ViewSortedRenderPhases,
};
use bevy_render::render_resource::TextureFormat::Rgba32Float;
use bevy_render::render_resource::binding_types::texture_2d;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayout, Extent3d,
    ShaderStages, TextureDescriptor, TextureDimension,
    TextureSampleType, TextureUsages, TextureView,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::texture::{ColorAttachment, TextureCache};
use bevy_render::view::ExtractedView;

#[derive(Component)]
pub struct ViewAttributePrepassTextures {
    pub attribute: Option<ColorAttachment>,
    pub size: Extent3d,
}

impl ViewAttributePrepassTextures {
    pub fn attribute_view(&self) -> Option<&TextureView> {
        self.attribute.as_ref().map(|t| &t.texture.default_view)
    }
}

pub fn prepare_attribute_pass_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    opaque_3d_phases: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha_mask_3d_phases: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    transmissive_3d_phases: Res<ViewSortedRenderPhases<Transmissive3d>>,
    transparent_3d_phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views_3d: Query<(Entity, &ExtractedCamera, &ExtractedView, &Msaa)>,
) {
    let mut textures = <HashMap<_, _>>::default();
    for (entity, camera, extracted_view, msaa) in &views_3d {
        if !opaque_3d_phases.contains_key(&extracted_view.retained_view_entity)
            || !alpha_mask_3d_phases.contains_key(&extracted_view.retained_view_entity)
            || !transmissive_3d_phases.contains_key(&extracted_view.retained_view_entity)
            || !transparent_3d_phases.contains_key(&extracted_view.retained_view_entity)
        {
            continue;
        };

        let Some(physical_target_size) = camera.physical_target_size else {
            continue;
        };

        let size = Extent3d {
            depth_or_array_layers: 1,
            width: physical_target_size.x,
            height: physical_target_size.y,
        };

        let cached_texture = textures
            .entry((camera.target.clone(), msaa))
            .or_insert_with(|| {
                // The size of the depth texture
                let size = Extent3d {
                    depth_or_array_layers: 1,
                    width: physical_target_size.x,
                    height: physical_target_size.y,
                };

                let descriptor = TextureDescriptor {
                    label: Some("pcl_attribute_pass_texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: msaa.samples(),
                    dimension: TextureDimension::D2,
                    format: Rgba32Float,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                };

                texture_cache.get(&render_device, descriptor)
            })
            .clone();

        commands
            .entity(entity)
            .insert(ViewAttributePrepassTextures {
                attribute: Some(ColorAttachment::new(
                    cached_texture,
                    None,
                    Some(LinearRgba::NONE),
                )),
                size,
            });
    }
}

#[derive(Component)]
pub struct AttributePassViewBindGroup {
    pub value: BindGroup,
}

#[derive(Resource)]
pub struct AttributePassLayout {
    pub layout: BindGroupLayout,
}

impl FromWorld for AttributePassLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        AttributePassLayout {
            layout: render_device.create_bind_group_layout(
                "pcl_attribute_layout",
                &vec![
                    texture_2d(TextureSampleType::Float { filterable: true })
                        .build(0, ShaderStages::FRAGMENT),
                ],
            ),
        }
    }
}

pub fn prepare_attribute_pass_bind_groups(
    mut commands: Commands,
    pipeline: Res<AttributePassPipeline>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ViewDepthPrepassTextures, &Msaa)>,
) {
    for (entity, prepass_textures, msaa) in &views {
        let Some(depth_texture) = &prepass_textures.depth else {
            warn!("No depth pass texture for {}", entity);
            continue;
        };

        let depth_view = depth_texture.texture.default_view.clone();

        let layout = match msaa.samples() {
            1 => vec![&pipeline.layout],
            _ => vec![&pipeline.layout_msaa],
        };

        commands.entity(entity).insert(AttributePassViewBindGroup {
            value: render_device.create_bind_group(
                "pcl_normalize_pass_view_bind_group",
                layout[0],
                &BindGroupEntries::single(&depth_view),
            ),
        });
    }
}

pub struct SetAttributePassTextures<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetAttributePassTextures<I> {
    type Param = ();
    type ViewQuery = &'static AttributePassViewBindGroup;
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        attribute_pass_view_bind_group: ROQueryItem<'w, Self::ViewQuery>,
        _entity: Option<ROQueryItem<'w, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &attribute_pass_view_bind_group.value, &[]);

        RenderCommandResult::Success
    }
}
