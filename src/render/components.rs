use crate::pointcloud::PointCloud;
use bevy_asset::{AsAssetId, AssetId, Assets, Handle, RenderAssetUsages};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::HookContext;
use bevy_ecs::query::QueryItem;
use bevy_ecs::world::DeferredWorld;
use bevy_ecs::{component::Component, reflect::ReflectComponent};
use bevy_reflect::{Reflect, std_traits::ReflectDefault};
use bevy_render::extract_component::ExtractComponent;
use bevy_render::mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_transform::prelude::GlobalTransform;
use crate::render::custom_uniform::CustomUniform;

const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Clone, PartialEq)]
#[component(on_add = add_pointcloud_mesh::<PointCloud3d>)]
pub struct PointCloud3d(pub Handle<PointCloud>);

impl ExtractComponent for PointCloud3d {
    type QueryData = (&'static PointCloud3d, &'static GlobalTransform);
    type QueryFilter = ();
    type Out = (PointCloud3d, CustomUniform);

    fn extract_component((point_cloud_3d, global_transform): QueryItem<'_, Self::QueryData>) -> Option<Self::Out> {
        let custom_uniform = CustomUniform {
            world_from_local: global_transform.compute_matrix(),
        };
        Some((point_cloud_3d.clone(), custom_uniform))
    }
}

impl From<PointCloud3d> for AssetId<PointCloud> {
    fn from(pointcloud: PointCloud3d) -> Self {
        pointcloud.id()
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
