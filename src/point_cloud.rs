use bevy_asset::{AsAssetId, Asset, AssetId, Assets, Handle, RenderAssetUsages};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::{Component, HookContext};
use bevy_ecs::reflect::ReflectComponent;
use bevy_ecs::world::DeferredWorld;
use bevy_math::Vec3;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bytemuck::{Pod, Zeroable};

const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Asset, Reflect)]
pub struct PointCloud {
    pub points: Vec<PointCloudData>,
}

#[derive(Debug, Clone, Copy, Reflect, Pod, Zeroable)]
#[repr(C)]
pub struct PointCloudData {
    pub position: Vec3,
    pub point_size: f32,
    pub color: [f32; 4],
}

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
#[component(on_add = add_pointcloud_mesh::<PointCloud3d>)]
pub struct PointCloud3d(pub Handle<PointCloud>);

impl From<PointCloud3d> for AssetId<PointCloud> {
    fn from(point_cloud: PointCloud3d) -> Self {
        point_cloud.id()
    }
}

impl From<&PointCloud3d> for AssetId<PointCloud> {
    fn from(pointcloud: &PointCloud3d) -> Self {
        pointcloud.id()
    }
}

impl AsAssetId for PointCloud3d {
    type Asset = PointCloud;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

/// A generic component add hook that automatically adds the appropriate
/// [`Mesh`] to an entity for point cloud rendering.
///
/// To use this
/// hook, add it to your renderable component like this:
///
/// ```ignore
/// #[derive(Component)]
/// #[component(on_add = add_pointcloud_mesh::<MyComponent>)]
/// struct MyComponent {
///     ...
/// }
/// ```
pub fn add_pointcloud_mesh<C>(mut world: DeferredWorld<'_>, HookContext { entity, .. }: HookContext)
where
    C: 'static,
{
    if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, QUAD_POSITIONS.to_vec())
        .with_inserted_indices(Indices::U32(QUAD_INDICES.to_vec()));

        // instantiate a new mesh for each point cloud to prevent bevy not rendering it thinking it's the same
        let mesh_handle = meshes.add(mesh);

        world.commands().entity(entity).insert(Mesh3d(mesh_handle));
    }
}
