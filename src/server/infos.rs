use bevy::{platform::collections::HashMap, tasks::Task};

use crate::TaskId;

#[derive(Default)]
pub(crate) struct PointCloudServerInfos {
    pub(crate) pending_tasks: HashMap<TaskId, Task<()>>,
}
