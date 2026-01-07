use serde_json::Value;
use std::collections::HashMap;

pub fn deep_merge_json_maps(target: &mut HashMap<String, Value>, overlay: &HashMap<String, Value>) {
    for (key, value) in overlay {
        match target.get_mut(key) {
            Some(existing) => deep_merge_json_value(existing, value),
            None => {
                target.insert(key.clone(), value.clone());
            },
        }
    }
}

fn deep_merge_json_value(target: &mut Value, overlay: &Value) {
    match (target, overlay) {
        (Value::Object(target_obj), Value::Object(overlay_obj)) => {
            for (key, overlay_value) in overlay_obj {
                match target_obj.get_mut(key) {
                    Some(existing_value) => deep_merge_json_value(existing_value, overlay_value),
                    None => {
                        target_obj.insert(key.clone(), overlay_value.clone());
                    },
                }
            }
        },
        (target_value, overlay_value) => {
            *target_value = overlay_value.clone();
        },
    }
}

