use uuid::Uuid;

#[derive(Debug)]
pub struct BehaviorTriggerRequest {
    pub instance_id: Uuid,
    pub action: BehaviorTriggerAction,
}

#[derive(Debug)]
pub enum BehaviorTriggerAction {
    ScheduledStart,
    ManualStart,
    Stop,
}
