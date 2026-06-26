use bevy::{
    asset::Handle,
    ecs::{component::Component, query::QueryItem},
    mesh::Mesh,
    render::{extract_component::ExtractComponent, sync_component::SyncComponent},
};

#[derive(Component, Clone)]
pub struct PointCloud {
    pub quad_mesh: Handle<Mesh>,
    pub points_mesh: Handle<Mesh>,
}

impl SyncComponent for PointCloud {
    type Target = Self;
}

impl ExtractComponent for PointCloud {
    type QueryData = &'static PointCloud;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}
