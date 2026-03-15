use crate::octree::extract::render::components::RenderVisibleOctreeNodes;
use crate::octree::extract::render::resources::{RenderOctreeIndex, RenderOctrees};
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
use bevy_log::warn;
use bevy_platform::collections::HashMap;
use bevy_render::camera::ExtractedCamera;
use bevy_render::prelude::*;
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use bevy_render::render_resource::TextureFormat::Rgba8Uint;
use bevy_render::render_resource::binding_types::texture_2d;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayout, Extent3d, ShaderStages, TexelCopyBufferLayout,
    TextureDescriptor, TextureDimension, TextureSampleType, TextureUsages,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::texture::{ColorAttachment, TextureCache};
use bevy_render::view::ExtractedView;
use bytemuck::{Pod, Zeroable};
use std::cmp::Ordering;

/// Stores visible nodes and mapping textures for each view
#[derive(Component)]
pub struct VisibleNodesTexture {
    pub visible_nodes: Option<ColorAttachment>,
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
        &ExtractedView,
        &Msaa,
        &RenderVisibleOctreeNodes<PointCloudNodeData, PointCloudOctree3d>,
    )>,
    mut visible_nodes_buffer: Local<Vec<VisibleOctreeNodeUniform>>,
    render_octrees: Res<RenderOctrees<RenderPointCloudNodeData>>,
) {
    // for each camera
    for (entity, extracted_view, _msaa, visible_nodes) in &views_3d {
        // skip if no phases
        if !point_cloud_octree_3d_node_depth_phases
            .contains_key(&extracted_view.retained_view_entity)
            || !point_cloud_octree_3d_node_attribute_phases
                .contains_key(&extracted_view.retained_view_entity)
        {
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

        // Get the texture for containing visible nodes data
        let visible_nodes_texture = {
            // The size of the depth texture
            let size = Extent3d {
                width: MAX_NODES as u32, // max 2048 nodes per octree visible at the same time
                height: octrees_count as u32,
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

            let Some(render_octree) = render_octrees.get(*asset_id) else {
                warn!(
                    "Render Point Cloud octree {} not found in RenderOctrees, skip",
                    entity
                );
                continue;
            };

            let mut sorted_octree_nodes = octree_nodes.clone();
            // remove the missing nodes
            sorted_octree_nodes.retain(|node| render_octree.nodes.contains_key(&node.id));
            // sort nodes in order of depth, then child index ordering
            sorted_octree_nodes.sort_by(|a, b| {
                if a.depth < b.depth {
                    return Ordering::Less;
                }
                if a.depth > b.depth {
                    return Ordering::Greater;
                }

                a.name.cmp(&b.name)
            });

            // recompute children mask
            for node in &mut sorted_octree_nodes {
                for child_index in iter_one_bits(node.children_mask) {
                    let index = node.children[child_index as usize];
                    let child_node_id = octree_nodes[index].id;

                    // if the node is missing in the render octree, patch the children mask
                    if !render_octree.nodes.contains_key(&child_node_id) {
                        node.children_mask &= !(1u8 << child_index);
                    }
                }
            }

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

                let Some(node) = render_octree.nodes.get(&visible_node.id) else {
                    bevy_log::warn!(
                        "Render Point Cloud Octree node {:?} not found in RenderOctrees, skip.",
                        visible_node.id
                    );
                    continue;
                };

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
    // pub uniform_layout: BindGroupLayout,
}

pub const MAX_NODES: usize = 2048;

impl FromWorld for VisibleNodesTextureLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        VisibleNodesTextureLayout {
            layout: render_device.create_bind_group_layout(
                "pcl_attribute_layout",
                &vec![texture_2d(TextureSampleType::Uint).build(0, ShaderStages::VERTEX)],
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

        commands
            .entity(entity)
            .insert(VisibleNodesTextureBindGroup {
                texture: render_device.create_bind_group(
                    "pcl_octree_visible_nodes__bind_group",
                    &visible_nodes_texture_layout.layout,
                    &BindGroupEntries::single(&texture_view),
                ),
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
