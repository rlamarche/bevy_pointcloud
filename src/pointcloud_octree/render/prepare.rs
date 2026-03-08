use super::phase::PointCloudOctreeBinnedPhaseItem;
use crate::octree::extract::{RenderOctreeIndex, RenderOctrees, RenderVisibleOctreeNodes};
use crate::octree::storage::NodeId;
use crate::octree::visibility::iter_one_bits;
use crate::pointcloud_octree::asset::data::PointCloudNodeData;
use crate::pointcloud_octree::component::PointCloudOctree3d;
use crate::pointcloud_octree::extract::RenderPointCloudNodeData;
use crate::pointcloud_octree::render::phase::{
    PointCloudOctree3dNodePhase, ViewOctreeNodesRenderAttributePhases,
    ViewOctreeNodesRenderDepthPhases,
};
use bevy_color::LinearRgba;
use bevy_ecs::prelude::*;
use bevy_ecs::query::ROQueryItem;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::{Read, SRes};
use bevy_log::warn;
use bevy_platform::collections::HashMap;
use bevy_render::camera::ExtractedCamera;
use bevy_render::prelude::*;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::TextureFormat::Rgba8Uint;
use bevy_render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntries,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBinding, BufferBindingType,
    BufferInitDescriptor, BufferSize, BufferUsages, Extent3d, ShaderStages, ShaderType,
    TexelCopyBufferLayout, TextureDescriptor, TextureDimension, TextureSampleType, TextureUsages,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::texture::{ColorAttachment, TextureCache};
use bevy_render::view::ExtractedView;
use bytemuck::{Pod, Zeroable};
use std::cmp::Ordering;

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
    /// contains node index per octree index (see [`RenderOctreeIndex`])
    pub node_index: Vec<HashMap<NodeId, u32>>,
}

/// The data layout for the texture containing visible nodes data
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct VisibleOctreeNodeUniform {
    // the children mask
    pub children_mask: u8,
    pub offset: u8,
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

#[derive(Clone, Debug)]
pub struct PreparedVisibleOctreeNode {
    pub id: NodeId,
    pub children_mask: u8,
    pub first_child_index: u16,
}

pub fn prepare_visible_nodes_texture(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_octree_index: Res<RenderOctreeIndex<PointCloudOctree3d>>,
    point_cloud_octree_3d_node_depth_phases: Res<
        ViewOctreeNodesRenderAttributePhases<PointCloudOctree3dNodePhase>,
    >,
    point_cloud_octree_3d_node_attribute_phases: Res<
        ViewOctreeNodesRenderDepthPhases<PointCloudOctree3dNodePhase>,
    >,
    views_3d: Query<(
        Entity,
        &ExtractedCamera,
        &ExtractedView,
        &Msaa,
        &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
    )>,
    mut visible_nodes_buffer: Local<Vec<VisibleOctreeNodeUniform>>,
    render_octrees: Res<RenderOctrees<RenderPointCloudNodeData>>,
) {
    for (entity, camera, extracted_view, _msaa, visible_nodes) in &views_3d {
        if !point_cloud_octree_3d_node_depth_phases
            .contains_key(&extracted_view.retained_view_entity)
            || !point_cloud_octree_3d_node_attribute_phases
                .contains_key(&extracted_view.retained_view_entity)
        {
            continue;
        };

        let Some(physical_target_size) = camera.physical_target_size else {
            continue;
        };

        let octrees_count = visible_nodes.octrees.len();

        if octrees_count == 0 {
            continue;
        }

        let required_buffer_size = octrees_count * MAX_NODES;

        // prepare the buffer only once
        if visible_nodes_buffer.len() < required_buffer_size {
            visible_nodes_buffer.resize(required_buffer_size, VisibleOctreeNodeUniform::default());
        }

        let size = Extent3d {
            depth_or_array_layers: 1,
            width: physical_target_size.x,
            height: physical_target_size.y,
        };

        // Get the texture for containing visible nodes data
        let visible_nodes_texture = {
            // The size of the depth texture
            let size = Extent3d {
                width: MAX_NODES as u32, // max 2048 nodes per octree visible at the same time
                height: octrees_count as u32, // max 64 octrees visibles at the same time
                depth_or_array_layers: 1,
            };

            let descriptor = TextureDescriptor {
                label: Some("pcl_visible_nodes_texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: Rgba8Uint,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            };

            texture_cache.get(&render_device, descriptor)
        };

        let mut node_index =
            vec![HashMap::<NodeId, u32>::default(); render_octree_index.octrees_slab.len()];

        for (entity, (asset_id, octree_nodes)) in &visible_nodes.octrees {
            let octree_index = render_octree_index
                .get_octree_index(*entity)
                .expect("octree index out of bounds");

            let node_mapping = &mut node_index[octree_index];

            let base_offset = octree_index * MAX_NODES;

            let Some(octree) = render_octrees.get(*asset_id) else {
                warn!(
                    "Render Point Cloud octree {} not found in RenderOctrees, skip",
                    entity
                );
                continue;
            };

            // sort nodes in order or depth, then child index ordering
            let mut sorted_octree_nodes = octree_nodes.clone();
            sorted_octree_nodes.sort_by(|a, b| {
                if a.depth < b.depth {
                    return Ordering::Less;
                }
                if a.depth > b.depth {
                    return Ordering::Greater;
                }

                a.name.cmp(&b.name)
            });

            // create an index of child indexes for each parent
            let index = sorted_octree_nodes
                .iter()
                .enumerate()
                .map(|(index, node)| ((node.parent_id, node.child_index), index))
                .collect::<HashMap<_, _>>();

            // prepare octree nodes
            let prepared_octree_nodes = sorted_octree_nodes
                .clone()
                .into_iter()
                .map(|node| {
                    let mut first_child_index = 0;
                    // get the first child index
                    if let Some(child_index) = iter_one_bits(node.children_mask).next() {
                        if let Some(index) = index.get(&(Some(node.id), child_index)) {
                            first_child_index = *index as u16;
                        } else {
                            warn!("No first child index found in index");
                        }
                    }
                    PreparedVisibleOctreeNode {
                        id: node.id,
                        children_mask: node.children_mask,
                        first_child_index,
                    }
                })
                .collect::<Vec<_>>();

            for (i, visible_node) in prepared_octree_nodes.iter().enumerate() {
                if i >= MAX_NODES {
                    // warn!("Too many nodes in octree, some will be ignored.");
                    break;
                }

                let Some(node) = octree.nodes.get(&visible_node.id) else {
                    warn!(
                        "Render Point Cloud Octree node {:?} not found in RenderOctrees, skip.",
                        visible_node.id
                    );
                    continue;
                };

                // let offset = ((node.data.offset + 10.0) * 10.0) as u8;
                let offset = ((node.data.offset + 10.0) * 10.0).min(255.0) as u8;

                visible_nodes_buffer[base_offset + i] = VisibleOctreeNodeUniform {
                    children_mask: visible_node.children_mask,
                    offset,
                    first_child_index: visible_node.first_child_index,
                };

                node_mapping.insert(visible_node.id, i as u32);
            }
        }

        render_queue.write_texture(
            visible_nodes_texture.texture.as_image_copy(),
            bytemuck::cast_slice(&visible_nodes_buffer),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((MAX_NODES * 4) as u32), // 4 bytes par texel RGBA8Uint
                rows_per_image: Some(octrees_count as u32),
            },
            Extent3d {
                width: MAX_NODES as u32,
                height: octrees_count as u32,
                depth_or_array_layers: 1,
            },
        );

        commands.entity(entity).insert(VisibleNodesTexture {
            visible_nodes: Some(ColorAttachment::new(
                visible_nodes_texture,
                None,
                Some(LinearRgba::NONE),
            )),
            size,
            // octree_mapping,
            // octree_index,
            node_index,
        });
    }
}

#[derive(Component)]
pub struct VisibleNodesTextureBindGroup {
    pub texture: BindGroup,
}

#[derive(Resource)]
pub struct VisibleNodesTextureLayout {
    pub layout: BindGroupLayout,
    pub uniform_layout: BindGroupLayout,
}

const BUFFER_SIZE: usize = 65536;
pub const MAX_NODES: usize = 2048;

#[derive(Resource)]
pub struct OctreeNodesMappingBindGroups {
    pub layout: BindGroupLayout,
    // bind groups for each octree indexes
    pub bind_groups: Vec<Vec<BindGroup>>,
    pub min_uniform_buffer_offset_alignment: usize,
    pub max_nodes_per_buffer: usize,
    pub nb_buffers: usize,
}

impl FromWorld for OctreeNodesMappingBindGroups {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            Some("layout_node_mapping"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(64),
                },
                count: None,
            }],
        );

        // let array: [u32; 2048] = core::array::from_fn(|i| (i + 1) as u32);
        // let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        //     label: Some("node_mapping_buffer"),
        //     contents: bytemuck::cast_slice(&array),
        //     usage: BufferUsages::UNIFORM,
        // });
        //
        //
        // let bind_group = render_device.create_bind_group(
        //     "bind_group_node_mapping",
        //     &layout,
        //     &[BindGroupEntry {
        //         binding: 0,
        //         resource: buffer.as_entire_binding(),
        //     }],
        // );

        let min_uniform_buffer_offset_alignment = render_device
            .wgpu_device()
            .limits()
            .min_uniform_buffer_offset_alignment
            as usize
            * 2; // for android compat we multiply by 2
        // TODO: create a special case ?
        let max_nodes_per_buffer = BUFFER_SIZE / min_uniform_buffer_offset_alignment;
        let nb_buffers = MAX_NODES / max_nodes_per_buffer;

        Self {
            layout,
            bind_groups: Vec::new(),
            min_uniform_buffer_offset_alignment,
            max_nodes_per_buffer,
            nb_buffers,
        }
    }
}

/// The octree node mapping uniform layout
#[derive(Clone, Debug, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct OctreeNodeMapping {
    pub octree_index: u32,
    pub node_index: u32,
    pub _padding: [u8; 56],
}

pub fn prepare_octree_nodes_mapping_buffers(
    render_octree_index: Res<RenderOctreeIndex<PointCloudOctree3d>>,
    mut mappings: ResMut<OctreeNodesMappingBindGroups>,
    render_device: Res<RenderDevice>,
) {
    // add missing octree buffers
    for i in mappings.bind_groups.len()..render_octree_index.octrees_slab.len() {
        let mut buffer_data: Vec<u8> = Vec::with_capacity(BUFFER_SIZE * mappings.nb_buffers);

        for node_idx in 0..MAX_NODES {
            let data = [i as u32, node_idx as u32];

            buffer_data.extend_from_slice(bytemuck::cast_slice(&data));
            buffer_data.resize(
                buffer_data.len() + (mappings.min_uniform_buffer_offset_alignment - 8),
                0,
            );
        }

        let mut bind_groups = Vec::with_capacity(mappings.nb_buffers);
        for j in 0..mappings.nb_buffers {
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("node_mapping_buffer"),
                contents: &buffer_data[j * BUFFER_SIZE..(j + 1) * BUFFER_SIZE],
                usage: BufferUsages::UNIFORM,
            });
            let bind_group = render_device.create_bind_group(
                "bind_group_node_mapping",
                &mappings.layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: BufferSize::new(mappings.min_uniform_buffer_offset_alignment as u64),
                    }),
                }],
            );

            bind_groups.push(bind_group);
        }

        // let buffer_1 = render_device.create_buffer_with_data(&BufferInitDescriptor {
        //     label: Some("node_mapping_buffer"),
        //     contents: bytemuck::cast_slice(&array[0..1024]),
        //     usage: BufferUsages::UNIFORM,
        // });
        // let buffer_2 = render_device.create_buffer_with_data(&BufferInitDescriptor {
        //     label: Some("node_mapping_buffer"),
        //     contents: bytemuck::cast_slice(&array[1024..2048]),
        //     usage: BufferUsages::UNIFORM,
        // });
        // println!("Buffer 1 size: {}", buffer_1.size());
        // println!("Buffer 2 size: {}", buffer_2.size());

        // let bind_group_1 = render_device.create_bind_group(
        //     "bind_group_node_mapping",
        //     &mappings.layout,
        //     &[BindGroupEntry {
        //         binding: 0,
        //         resource: BindingResource::Buffer(BufferBinding {
        //             buffer: &buffer_1,
        //             offset: 0,
        //             size: BufferSize::new(64),
        //         }),
        //     }],
        // );
        // let bind_group_2 = render_device.create_bind_group(
        //     "bind_group_node_mapping",
        //     &mappings.layout,
        //     &[BindGroupEntry {
        //         binding: 0,
        //         resource: BindingResource::Buffer(BufferBinding {
        //             buffer: &buffer_2,
        //             offset: 0,
        //             size: BufferSize::new(64),
        //         }),
        //     }],
        // );

        mappings.bind_groups.push(bind_groups);
    }
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
    for (entity, prepass_textures, _msaa) in &views {
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

pub struct SetVisibleOctreeUniformGroup<const I: usize>;
impl<P: PointCloudOctreeBinnedPhaseItem, const I: usize> RenderCommand<P>
    for SetVisibleOctreeUniformGroup<I>
{
    type Param = (
        SRes<RenderOctreeIndex<PointCloudOctree3d>>,
        SRes<RenderOctrees<RenderPointCloudNodeData>>,
        SRes<OctreeNodesMappingBindGroups>,
    );
    type ViewQuery = Read<VisibleNodesTexture>;
    type ItemQuery = Read<PointCloudOctree3d>;

    fn render<'w>(
        item: &P,
        visible_nodes: ROQueryItem<'w, '_, Self::ViewQuery>,
        point_cloud_octree_3d: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (render_octree_index, render_octrees, octree_nodes_mapping_bind_groups): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let render_octree_index = render_octree_index.into_inner();
        let render_octrees = render_octrees.into_inner();
        let octree_nodes_mapping_bind_groups = octree_nodes_mapping_bind_groups.into_inner();

        let Some(point_cloud_octree_3d) = point_cloud_octree_3d else {
            warn!("Missing point cloud octree 3d item");
            return RenderCommandResult::Skip;
        };

        let Some(_octree) = render_octrees.get(point_cloud_octree_3d) else {
            warn!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        let Some(octree_index) = render_octree_index.get_octree_index(item.entity()) else {
            warn!("Missing octree when render");
            return RenderCommandResult::Skip;
        };

        // get the octree's bind group
        let bind_group = &octree_nodes_mapping_bind_groups.bind_groups[octree_index];

        let node_id = item.node_id();

        // get the node's index
        let Some(node_index) = visible_nodes.node_index[octree_index].get(&node_id) else {
            // warn!("Missing node index when render");
            // it happens when there is more than 2048 nodes to render
            return RenderCommandResult::Skip;
        };

        // we are ready to bind the correct bind group with the correct dynamic offset

        let buffer_index =
            *node_index as usize / octree_nodes_mapping_bind_groups.max_nodes_per_buffer;
        let remapped_node_index =
            *node_index % octree_nodes_mapping_bind_groups.max_nodes_per_buffer as u32;

        let min_uniform_buffer_offset_alignment =
            octree_nodes_mapping_bind_groups.min_uniform_buffer_offset_alignment as u32;

        pass.set_bind_group(
            I,
            &bind_group[buffer_index],
            &[remapped_node_index * min_uniform_buffer_offset_alignment],
        );

        // match node_index {
        //     0..1024 => {
        //         pass.set_bind_group(
        //             I,
        //             &bind_group[0],
        //             &[node_index * min_uniform_buffer_offset_alignment],
        //         );
        //     }
        //     1024..2048 => {
        //         pass.set_bind_group(
        //             I,
        //             &bind_group[1],
        //             &[(node_index * min_uniform_buffer_offset_alignment) - 1024],
        //         );
        //     }
        //     _ => {
        //         warn!("Node index out of bounds: {}, skip", node_index);
        //         return RenderCommandResult::Skip;
        //     }
        // }

        RenderCommandResult::Success
    }
}
