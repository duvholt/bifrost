use serde_json::Value;
use std::collections::BTreeMap;

use crate::common::{TestBridge, TestResult};

pub mod common;

const fn z2m_state() -> BTreeMap<String, Value> {
    BTreeMap::new()
}

#[tokio::test]
async fn read_config() -> TestResult<()> {
    let test_bridge = TestBridge::start(z2m_state()).await?;

    let response = reqwest::get(test_bridge.hue_url.join("/api/config").unwrap())
        .await
        .unwrap();

    assert!(response.status().is_success());
    Ok(())
}
