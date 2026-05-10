use bevy_ecs::message::Message;

/// Fired when the user requests a new point cloud to be loaded.
#[derive(Message)]
pub struct LoadPointCloudMessage {
    pub url: String,
}
