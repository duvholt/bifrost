use serde_json::Value;

use hue::api::{BehaviorInstance, Resource, ResourceLink};
use hue::api::{BehaviorInstanceNew, BehaviorInstanceUpdate};
use uuid::Uuid;

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn post_behavior_instance(state: &AppState, post: Value) -> ApiV2Result {
    let new: BehaviorInstanceNew = serde_json::from_value(post)?;

    let obj = Resource::BehaviorInstance(BehaviorInstance {
        dependees: vec![],
        enabled: new.enabled,
        last_error: None,
        metadata: new.metadata,
        script_id: new.script_id,
        status: None,
        state: None,
        migrated_from: None,
        configuration: new.configuration,
    });

    let rlink = ResourceLink::new(Uuid::new_v4(), obj.rtype());

    state.res.lock().await.add(&rlink, obj)?;

    V2Reply::ok(rlink)
}

pub async fn put_behavior_instance(
    state: &AppState,
    rlink: ResourceLink,
    put: Value,
) -> ApiV2Result {
    let upd: BehaviorInstanceUpdate = serde_json::from_value(put)?;

    state
        .res
        .lock()
        .await
        .update::<BehaviorInstance>(&rlink.rid, |bi| *bi += upd)?;

    V2Reply::ok(rlink)
}
