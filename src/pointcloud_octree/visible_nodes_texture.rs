use crate::octree::asset::NodeId;
use crate::octree::visibility::RenderVisibleOctreeNodes;
use crate::pointcloud_octree::asset::PointCloudNodeData;
use crate::pointcloud_octree::render::attribute_pass::phase::PointCloudOctree3dAttributePhase;
use crate::pointcloud_octree::render::depth_pass::phase::PointCloudOctree3dDepthPhase;
use bevy_color::LinearRgba;
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::SystemParamItem;
use bevy_log::warn;
use bevy_platform::collections::HashMap;
use bevy_render::camera::ExtractedCamera;
use bevy_render::prelude::*;
use bevy_render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass, ViewBinnedRenderPhases,
};
use bevy_render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy_render::render_resource::TextureFormat::Rgba8Uint;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, Extent3d, ShaderStages,
    ShaderType, TexelCopyBufferLayout, TextureDescriptor, TextureDimension, TextureSampleType,
    TextureUsages,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::texture::{ColorAttachment, TextureCache};
use bevy_render::view::ExtractedView;
use bytemuck::{Pod, Zeroable};

#[derive(ShaderType)]
pub struct PointCloudVisibleNodeUniform {
    pub index: u32,
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _padding_1: bevy_math::Vec3,
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    pub _padding_2: bevy_math::Vec4,
}

/// Stores visible nodes and mapping textures for each view
#[derive(Component)]
pub struct VisibleNodesTexture {
    pub visible_nodes: Option<ColorAttachment>,
    pub size: Extent3d,
    pub octree_mapping: HashMap<Entity, HashMap<NodeId, usize>>,
    pub octree_index: HashMap<Entity, (u32, HashMap<NodeId, u32>)>,
}

/// The data layout for the texture containing visible nodes data
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct VisibleOctreeNodeUniform {
    // the children mask
    pub children_mask: u8,
    pub _padding: u8,
    // index of the first child
    pub first_child_index: u16,
}

impl ::core::fmt::Debug for VisibleOctreeNodeUniform {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(
            f,
            "mask = {:b} first_child_index = {:?}",
            self.children_mask, self.first_child_index
        )?;
        Ok(())
    }
}

pub fn prepare_visible_nodes_texture(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    point_cloud_octree_3d_attribute_phases: Res<
        ViewBinnedRenderPhases<PointCloudOctree3dAttributePhase>,
    >,
    point_cloud_octree_3d_depth_phases: Res<ViewBinnedRenderPhases<PointCloudOctree3dDepthPhase>>,
    views_3d: Query<(
        Entity,
        &ExtractedCamera,
        &ExtractedView,
        &Msaa,
        &RenderVisibleOctreeNodes<PointCloudNodeData>,
    )>,
    mut visible_nodes_buffer: Local<Vec<VisibleOctreeNodeUniform>>,
) {
    const MAX_NODES_PER_OCTREE: usize = 2048;
    const MAX_OCTREES: usize = 64;
    const REQUIRED_BUFFER_SIZE: usize = MAX_NODES_PER_OCTREE * MAX_OCTREES;

    // prepare the buffer only once
    if visible_nodes_buffer.len() < REQUIRED_BUFFER_SIZE {
        visible_nodes_buffer.resize(REQUIRED_BUFFER_SIZE, VisibleOctreeNodeUniform::default());
    }

    for (entity, camera, extracted_view, msaa, visible_nodes) in &views_3d {
        if !point_cloud_octree_3d_attribute_phases
            .contains_key(&extracted_view.retained_view_entity)
            || !point_cloud_octree_3d_depth_phases
                .contains_key(&extracted_view.retained_view_entity)
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

        // Get the texture for containing visible nodes data
        let visible_nodes_texture = {
            // The size of the depth texture
            let size = Extent3d {
                width: 2048, // max 2048 nodes per octree visible at the same time
                height: 64,  // max 64 octrees visibles at the same time
                depth_or_array_layers: 1,
            };

            let descriptor = TextureDescriptor {
                label: Some("pcl_visible_nodes_texture"),
                size,
                mip_level_count: 1,
                sample_count: msaa.samples(),
                dimension: TextureDimension::D2,
                format: Rgba8Uint,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            };

            texture_cache.get(&render_device, descriptor)
        };

        let mut octree_mapping = HashMap::<Entity, HashMap<NodeId, usize>>::new();
        let mut octree_index = HashMap::<Entity, (u32, HashMap<NodeId, u32>)>::new();

        let mut row_index = 0;
        'main_loop: for (entity, octree_nodes) in &visible_nodes.octrees {
            if row_index >= MAX_OCTREES {
                warn!("Too many octrees, some will be ignored.");
                break 'main_loop;
            }

            let node_mapping = octree_mapping.entry(*entity).or_default();
            let base_offset = row_index * MAX_NODES_PER_OCTREE;

            let mut node_index = HashMap::<NodeId, u32>::new();
            for (i, visible_node) in octree_nodes.iter().enumerate() {
                if i >= MAX_NODES_PER_OCTREE {
                    warn!("Too many nodes in octree, some will be ignored.");
                    break;
                }

                visible_nodes_buffer[base_offset + i] = VisibleOctreeNodeUniform {
                    children_mask: visible_node.children_mask,
                    _padding: 0,
                    first_child_index: visible_node.first_child_index as u16,
                };

                node_mapping.insert(visible_node.id, i);
                node_index.insert(visible_node.id, i as u32);
            }

            octree_index.insert(*entity, (row_index as u32, node_index));

            row_index += 1;
        }

        render_queue.write_texture(
            visible_nodes_texture.texture.as_image_copy(),
            bytemuck::cast_slice(&visible_nodes_buffer),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((MAX_NODES_PER_OCTREE * 4) as u32), // 4 bytes par texel RGBA8Uint
                rows_per_image: Some(MAX_OCTREES as u32),
            },
            Extent3d {
                width: MAX_NODES_PER_OCTREE as u32,
                height: MAX_OCTREES as u32,
                depth_or_array_layers: 1,
            },
        );

        // // TODO use Vec::with_capacity
        // let mut visible_nodes_buffer = Vec::<VisibleOctreeNodeUniform>::new();
        //
        // 'main_loop: for (entity, octree_nodes) in &visible_nodes.octrees {
        //     let root_index = visible_nodes_buffer.len();
        //
        //     let node_mapping = octree_mapping.entry(*entity).or_default();
        //
        //     for visible_node in octree_nodes {
        //         let current_index = visible_nodes_buffer.len();
        //
        //         if current_index == 2048 {
        //             // we have reached the max size of the texture
        //             warn!("Too many nodes, some will be ignored for the LOD structure.");
        //             break 'main_loop;
        //         }
        //
        //         visible_nodes_buffer.push(VisibleOctreeNodeUniform {
        //             children_mask: visible_node.children_mask,
        //             _padding: 0,
        //             first_child_index: if visible_node.first_child_index > 0 {
        //                 root_index + visible_node.first_child_index
        //             } else {
        //                 0
        //             } as u16,
        //         });
        //
        //         // add the mapping
        //         node_mapping.insert(visible_node.id, current_index);
        //     }
        // }
        //
        // // println!("{:#?}", visible_nodes_buffer);
        //
        // render_queue.write_texture(
        //     visible_nodes_texture.texture.as_image_copy(),
        //     bytemuck::cast_slice(&visible_nodes_buffer),
        //     TexelCopyBufferLayout {
        //         offset: 0,
        //         bytes_per_row: Some(2048 * 4), // 4 bytes par u32
        //         rows_per_image: Some(1),
        //     },
        //     Extent3d {
        //         width: visible_nodes_buffer.len() as u32,
        //         height: 1,
        //         depth_or_array_layers: 1,
        //     },
        // );

        commands.entity(entity).insert(VisibleNodesTexture {
            visible_nodes: Some(ColorAttachment::new(
                visible_nodes_texture,
                None,
                Some(LinearRgba::NONE),
            )),
            size,
            octree_mapping,
            octree_index,
        });
    }
}

#[derive(Component)]
pub struct VisibleNodesTextureBindGroup {
    pub texture: BindGroup,
    // pub octree_mappings: HashMap<Entity, HashMap<NodeId, BindGroup>>,
}

#[derive(Resource)]
pub struct VisibleNodesTextureLayout {
    pub layout: BindGroupLayout,
    pub uniform_layout: BindGroupLayout,
}

impl FromWorld for VisibleNodesTextureLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        VisibleNodesTextureLayout {
            layout: render_device.create_bind_group_layout(
                "pcl_attribute_layout",
                &vec![texture_2d(TextureSampleType::Uint).build(0, ShaderStages::VERTEX)],
            ),
            uniform_layout: render_device.create_bind_group_layout(
                "pcl_octree_node_data",
                &BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    uniform_buffer::<PointCloudVisibleNodeUniform>(false),
                ),
            ),
        }
    }
}

pub fn prepare_visible_nodes_texture_bind_group(
    mut commands: Commands,
    visible_nodes_texture_layout: Res<VisibleNodesTextureLayout>,
    render_device: Res<RenderDevice>,
    // render_queue: Res<RenderQueue>,
    views: Query<(Entity, &VisibleNodesTexture, &Msaa)>,
) {
    for (entity, prepass_textures, msaa) in &views {
        let Some(texture) = &prepass_textures.visible_nodes else {
            warn!("No visible nodes pass texture for {}", entity);
            continue;
        };

        let texture_view = texture.texture.default_view.clone();

        // let mut octree_mappings: HashMap<Entity, HashMap<NodeId, BindGroup>> = HashMap::new();
        // for (entity, node_mapping) in &prepass_textures.octree_mapping {
        //     let bind_group_node_mapping = octree_mappings.entry(*entity).or_default();
        //
        //     let mut a = HashMap::new();
        //     for (node_id, index) in node_mapping {
        //         a.insert(*node_id, *index);
        //
        //         let mut buffer = UniformBuffer::from(PointCloudVisibleNodeUniform {
        //             index: *index as u32,
        //             #[cfg(all(
        //                 feature = "webgl",
        //                 target_arch = "wasm32",
        //                 not(feature = "webgpu")
        //             ))]
        //             _padding_1: Default::default(),
        //             #[cfg(all(
        //                 feature = "webgl",
        //                 target_arch = "wasm32",
        //                 not(feature = "webgpu")
        //             ))]
        //             _padding_2: Default::default(),
        //         });
        //         buffer.write_buffer(&render_device, &render_queue);
        //
        //         let bind_group = render_device.create_bind_group(
        //             "pcl_pointcloud_octree_node_data",
        //             &visible_nodes_texture_layout.uniform_layout,
        //             &BindGroupEntries::single(buffer.binding().unwrap()),
        //         );
        //
        //         bind_group_node_mapping.insert(*node_id, bind_group);
        //     }
        //
        //     // println!("{:#?}", a);
        // }

        commands
            .entity(entity)
            .insert(VisibleNodesTextureBindGroup {
                texture: render_device.create_bind_group(
                    "pcl_octree_visible_nodes__bind_group",
                    &visible_nodes_texture_layout.layout,
                    &BindGroupEntries::single(&texture_view),
                ),
                // octree_mappings,
            });
    }
}

pub struct SetVisibleNodesTexture<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetVisibleNodesTexture<I> {
    type Param = ();
    type ViewQuery = &'static VisibleNodesTextureBindGroup;
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        attribute_pass_view_bind_group: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &attribute_pass_view_bind_group.texture, &[]);

        RenderCommandResult::Success
    }
}

// pub struct SetPointCloudVisibleUniformGroup<const I: usize>;
// impl<P: PhaseItem + PointCloudOctree3dPhase, const I: usize> RenderCommand<P>
//     for SetPointCloudVisibleUniformGroup<I>
// {
//     type Param = SRes<RenderOctrees<RenderPointCloudNodeData>>;
//     type ViewQuery = &'static VisibleNodesTextureBindGroup;
//     type ItemQuery = Read<PointCloudOctree3d>;
//
//     fn render<'w>(
//         item: &P,
//         visible_nodes: ROQueryItem<'w, '_, Self::ViewQuery>,
//         point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
//         render_octrees: SystemParamItem<'w, '_, Self::Param>,
//         pass: &mut TrackedRenderPass<'w>,
//     ) -> RenderCommandResult {
//         let render_octrees = render_octrees.into_inner();
//
//         let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
//             warn!("Missing point cloud octree 3d item");
//             return RenderCommandResult::Skip;
//         };
//
//         let Some(octree) = render_octrees.get(point_cloud_octree_3d) else {
//             warn!("Missing octree when render");
//             return RenderCommandResult::Skip;
//         };
//
//         let node_id = item.node_id();
//
//         let Some(node_mapping) = visible_nodes.octree_mappings.get(&item.entity()) else {
//             warn!("Missing octree mapping when render");
//             return RenderCommandResult::Skip;
//         };
//
//         let Some(bind_group) = node_mapping.get(&item.node_id()) else {
//             warn!("Missing node mapping when render");
//             return RenderCommandResult::Skip;
//         };
//
//         pass.set_bind_group(I, &bind_group, &[]);
//
//         RenderCommandResult::Success
//     }
// }
