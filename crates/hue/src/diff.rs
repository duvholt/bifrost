use std::collections::BTreeSet;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::error::{HueError, HueResult};

// These properties, if present, are always included, even when unchanged
const WHITELIST_KEYS: &[&str] = &["owner", "service_id"];

pub fn event_update_diff(ma: Value, mb: Value) -> HueResult<Option<Value>> {
    let (Value::Object(mut a), Value::Object(mut b)) = (ma, mb) else {
        return Err(HueError::Undiffable);
    };

    let mut diff = Map::new();

    // did we add any meaningful differences?
    let mut changed = false;

    // First, remove any whitelisted keys from both maps,
    // and prefer version from "b" value
    for key in WHITELIST_KEYS {
        let va = a.remove(*key);
        let vb = b.remove(*key);

        changed |= va != vb;

        if let Some(value) = vb.or(va) {
            diff.insert((*key).to_string(), value);
        }
    }

    let ka = a.keys().cloned().collect::<BTreeSet<String>>();
    let kb = b.keys().cloned().collect::<BTreeSet<String>>();

    // Keys that have appeared will be included
    for key in &kb - &ka {
        diff.insert(key.clone(), b.remove(&key).unwrap());
        changed = true;
    }

    // Keys that are common will be included, if changed
    for key in &ka & &kb {
        if a[&key] != b[&key] {
            diff.insert(key.clone(), b.remove(&key).unwrap());
            changed = true;
        }
    }

    if !changed {
        return Ok(None);
    }

    Ok(Some(Value::Object(diff)))
}

pub fn event_update_apply<T: Serialize + DeserializeOwned>(ma: &T, mb: Value) -> HueResult<T> {
    let ma = serde_json::to_value(ma)?;

    let (Value::Object(mut a), Value::Object(b)) = (ma, mb) else {
        return Err(HueError::Unmergable);
    };

    for (key, value) in b {
        a.insert(key, value);
    }

    Ok(serde_json::from_value(Value::Object(a))?)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use serde_json::json;

    use crate::diff::event_update_apply as apply;
    use crate::diff::event_update_diff as diff;
    use crate::error::HueError;

    #[test]
    fn diff_empty() {
        let a = json!({});
        let b = json!({});

        assert_eq!(diff(a, b).unwrap(), None);
    }

    #[test]
    fn diff_invalid() {
        let a = json!([]);
        let b = json!({});

        assert!(matches!(diff(a, b).unwrap_err(), HueError::Undiffable));
    }

    #[test]
    fn diff_value_unchanged() {
        let a = json!({"x": 42});
        let b = json!({"x": 42});

        assert_eq!(diff(a, b).unwrap(), None);
    }

    #[test]
    fn diff_whitelist_unchanged() {
        let a = json!({"owner": 42});
        let b = json!({"owner": 42});

        assert_eq!(diff(a, b).unwrap(), None);
    }

    #[test]
    fn diff_value_removed() {
        let a = json!({"x": 42});
        let b = json!({});

        assert_eq!(diff(a, b).unwrap(), None);
    }

    #[test]
    fn diff_value_added() {
        let a = json!({});
        let b = json!({"x": 42});
        let c = json!({"x": 42});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_value_changed() {
        let a = json!({"x": 17});
        let b = json!({"x": 42});
        let c = json!({"x": 42});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_whitelist_removed() {
        let a = json!({"owner": 17});
        let b = json!({});
        let c = json!({"owner": 17});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_whitelist_added() {
        let a = json!({});
        let b = json!({"owner": 17});
        let c = json!({"owner": 17});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_whitelist_changed() {
        let a = json!({"owner": 17});
        let b = json!({"owner": 42});
        let c = json!({"owner": 42});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_value_type_changed() {
        let a = json!({"x": 17});
        let b = json!({"x": "foo"});
        let c = json!({"x": "foo"});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn diff_whitelist_type_changed() {
        let a = json!({"owner": 17});
        let b = json!({"owner": "foo"});
        let c = json!({"owner": "foo"});

        assert_eq!(diff(a, b).unwrap(), Some(c));
    }

    #[test]
    fn apply_empty() {
        let a = json!({});
        let b = json!({});
        let c = json!({});

        assert_eq!(apply(&a, b).unwrap(), c);
    }

    #[test]
    fn apply_invalid() {
        let a = json!([]);
        let b = json!({});

        assert!(matches!(apply(&a, b).unwrap_err(), HueError::Unmergable));
    }

    #[test]
    fn apply_simply() {
        let a = json!({});
        let b = json!({"x": "y"});
        let c = json!({"x": "y"});

        assert_eq!(apply(&a, b).unwrap(), c);
    }

    #[test]
    fn apply_overwrite() {
        let a = json!({"x": "before"});
        let b = json!({"x": "after"});
        let c = json!({"x": "after"});

        assert_eq!(apply(&a, b).unwrap(), c);
    }

    #[test]
    fn apply_null() {
        let a = json!({"x": "before"});
        let b = json!({"x": Value::Null});
        let c = json!({"x": Value::Null});

        assert_eq!(apply(&a, b).unwrap(), c);
    }

    #[test]
    fn apply_some() {
        let a = json!({"x": "unchanged"});
        let b = json!({"x": "unchanged", "y": "new"});
        let c = json!({"x": "unchanged", "y": "new"});

        assert_eq!(apply(&a, b).unwrap(), c);
    }
}
