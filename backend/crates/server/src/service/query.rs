//! Shared query layer: alias resolution (FR-14). REST (both ports) and MCP
//! use this module, so aliases apply the same way in every interface.
//!
//! An alias maps a value of a dimension to another value. Dimensions are
//! `client_device` (values are JSON strings) and `parameter:<name>` for case
//! parameters (values keep their JSON type: `10` and `"10"` differ).

use std::collections::HashMap;

use serde_json::Value;
use sqlx::PgExecutor;

pub const DEVICE_DIMENSION: &str = "client_device";

pub fn parameter_dimension(name: &str) -> String {
    format!("parameter:{name}")
}

/// Key of a JSON scalar. Numbers compare by value, as in jsonb (`10` equals
/// `10.0`), strings and booleans by type and value.
fn key(value: &Value) -> String {
    match value {
        Value::Number(n) => format!("n:{}", n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => format!("s:{s}"),
        Value::Bool(b) => format!("b:{b}"),
        other => format!("x:{other}"),
    }
}

/// The current aliases, loaded once per request.
#[derive(Debug, Clone, Default)]
pub struct Aliases {
    /// dimension -> source key -> (source, target)
    by_dimension: HashMap<String, HashMap<String, (Value, Value)>>,
}

/// Longest alias chain followed (a -> b -> c ...). Longer chains and cycles
/// stop at the last value reached.
const MAX_CHAIN: usize = 32;

impl Aliases {
    pub async fn load(executor: impl PgExecutor<'_>) -> sqlx::Result<Self> {
        let rows: Vec<(String, Value, Value)> =
            sqlx::query_as("SELECT dimension, source_value, target_value FROM current_alias")
                .fetch_all(executor)
                .await?;
        Ok(Self::from_rows(rows))
    }

    pub fn from_rows(rows: impl IntoIterator<Item = (String, Value, Value)>) -> Self {
        let mut by_dimension: HashMap<String, HashMap<String, (Value, Value)>> = HashMap::new();
        for (dimension, source, target) in rows {
            by_dimension
                .entry(dimension)
                .or_default()
                .insert(key(&source), (source, target));
        }
        Self { by_dimension }
    }

    pub fn is_empty(&self) -> bool {
        self.by_dimension.is_empty()
    }

    /// The value after applying aliases, following chains.
    pub fn resolve(&self, dimension: &str, value: &Value) -> Value {
        let Some(map) = self.by_dimension.get(dimension) else {
            return value.clone();
        };
        let mut current = value.clone();
        let mut seen = vec![key(&current)];
        for _ in 0..MAX_CHAIN {
            match map.get(&key(&current)) {
                Some((_, target)) => {
                    let k = key(target);
                    if seen.contains(&k) {
                        break;
                    }
                    seen.push(k);
                    current = target.clone();
                }
                None => break,
            }
        }
        current
    }

    pub fn resolve_device(&self, name: &str) -> String {
        match self.resolve(DEVICE_DIMENSION, &Value::String(name.to_owned())) {
            Value::String(s) => s,
            other => other.to_string(),
        }
    }

    /// Applies the parameter aliases to every value of a parameter object.
    pub fn resolve_parameters(&self, parameters: &Value) -> Value {
        match parameters {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(name, v)| (name.clone(), self.resolve(&parameter_dimension(name), v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// All stored values that resolve to `value`: the value itself (unless
    /// it is aliased away) and every alias source whose chain ends there.
    /// Used to turn a filter on a resolved value into a filter on raw values.
    pub fn raw_values(&self, dimension: &str, value: &Value) -> Vec<Value> {
        let target = key(value);
        let mut out = Vec::new();
        if key(&self.resolve(dimension, value)) == target {
            out.push(value.clone());
        }
        if let Some(map) = self.by_dimension.get(dimension) {
            for (source, _) in map.values() {
                if key(source) != target && key(&self.resolve(dimension, source)) == target {
                    out.push(source.clone());
                }
            }
        }
        out
    }

    pub fn raw_device_names(&self, name: &str) -> Vec<String> {
        self.raw_values(DEVICE_DIMENSION, &Value::String(name.to_owned()))
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn aliases(rows: &[(&str, Value, Value)]) -> Aliases {
        Aliases::from_rows(
            rows.iter()
                .map(|(d, s, t)| ((*d).to_owned(), s.clone(), t.clone())),
        )
    }

    #[test]
    fn resolves_devices_and_chains() {
        let a = aliases(&[
            ("client_device", json!("pixel-8-a"), json!("pixel-8")),
            ("client_device", json!("pixel-8-old"), json!("pixel-8-a")),
        ]);
        assert_eq!(a.resolve_device("pixel-8-a"), "pixel-8");
        assert_eq!(a.resolve_device("pixel-8-old"), "pixel-8");
        assert_eq!(a.resolve_device("iphone"), "iphone");

        let mut raw = a.raw_device_names("pixel-8");
        raw.sort();
        assert_eq!(raw, vec!["pixel-8", "pixel-8-a", "pixel-8-old"]);
        // A name that is aliased away does not match itself.
        assert!(a.raw_device_names("pixel-8-a").is_empty());
    }

    #[test]
    fn parameter_aliases_keep_json_types() {
        let a = aliases(&[
            ("parameter:pipeline", json!("legacy"), json!("classic")),
            ("parameter:size", json!(10), json!(20)),
        ]);
        let params = json!({"pipeline": "legacy", "size": "10", "other": 10});
        // "10" (string) is not aliased, and `other` has no aliases.
        assert_eq!(
            a.resolve_parameters(&params),
            json!({"pipeline": "classic", "size": "10", "other": 10})
        );
        assert_eq!(a.resolve("parameter:size", &json!(10.0)), json!(20));
        assert_eq!(
            a.raw_values("parameter:size", &json!(20)),
            vec![json!(20), json!(10)]
        );
    }

    #[test]
    fn cycles_terminate() {
        let a = aliases(&[
            ("client_device", json!("a"), json!("b")),
            ("client_device", json!("b"), json!("a")),
        ]);
        assert_eq!(a.resolve_device("a"), "b");
        assert_eq!(a.resolve_device("b"), "a");
    }
}
