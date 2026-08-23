use next_domain::NextArtifact;
use serde_json::{Number, Value};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceComparison {
    pub equivalent: bool,
    pub first_difference: Option<String>,
}

#[derive(Debug, Error)]
pub enum PersistenceComparisonError {
    #[error("failed to project Next artifact to JSON for persistence comparison: {0}")]
    Json(#[from] serde_json::Error),
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => format!("bool({value})"),
        Value::Number(value) => format!("number({value})"),
        Value::String(value) => format!("string(len={})", value.chars().count()),
        Value::Array(value) => format!("array(len={})", value.len()),
        Value::Object(value) => format!("object(keys={})", value.len()),
    }
}

fn ordered_float_bits(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits & (1_u64 << 63) != 0 {
        !bits
    } else {
        bits | (1_u64 << 63)
    }
}

fn float_ulp_distance(left: f64, right: f64) -> Option<u64> {
    if !left.is_finite() || !right.is_finite() {
        return None;
    }
    if left == right {
        return Some(0);
    }
    Some(ordered_float_bits(left).abs_diff(ordered_float_bits(right)))
}

fn is_json_integer(number: &Number) -> bool {
    number.as_i64().is_some() || number.as_u64().is_some()
}

/// DDNX uses JSON for the renderer-independent document projection. Integer
/// values are exact. Finite floating-point values are considered persistence-
/// equivalent only when JSON serialization/deserialization changes them by at
/// most one IEEE-754 ULP. This narrowly covers known decimal round-trip drift
/// without masking meaningful geometry changes.
fn numbers_equivalent(left: &Number, right: &Number) -> bool {
    if left == right {
        return true;
    }
    if is_json_integer(left) || is_json_integer(right) {
        return false;
    }
    let (Some(left), Some(right)) = (left.as_f64(), right.as_f64()) else {
        return false;
    };
    matches!(float_ulp_distance(left, right), Some(0 | 1))
}

fn first_difference_at(left: &Value, right: &Value, path: &str) -> Option<String> {
    if value_kind(left) != value_kind(right) {
        return Some(format!(
            "{path}: type mismatch: {} vs {}",
            value_summary(left),
            value_summary(right)
        ));
    }

    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys() {
                if !right.contains_key(key) {
                    return Some(format!("{path}.{key}: key missing after round-trip"));
                }
            }
            for key in right.keys() {
                if !left.contains_key(key) {
                    return Some(format!("{path}.{key}: unexpected key after round-trip"));
                }
            }
            for (key, left_value) in left {
                let right_value = &right[key];
                if let Some(difference) =
                    first_difference_at(left_value, right_value, &format!("{path}.{key}"))
                {
                    return Some(difference);
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path}: array length mismatch: {} vs {}",
                    left.len(),
                    right.len()
                ));
            }
            for (index, (left_value, right_value)) in left.iter().zip(right).enumerate() {
                if let Some(difference) =
                    first_difference_at(left_value, right_value, &format!("{path}[{index}]"))
                {
                    return Some(difference);
                }
            }
            None
        }
        (Value::Number(left), Value::Number(right)) => {
            (!numbers_equivalent(left, right)).then(|| {
                format!(
                    "{path}: numeric mismatch beyond 1 ULP: {} vs {}",
                    value_summary(&Value::Number(left.clone())),
                    value_summary(&Value::Number(right.clone()))
                )
            })
        }
        (Value::String(left), Value::String(right)) => (left != right).then(|| {
            format!(
                "{path}: string value mismatch (lengths {} vs {}; contents suppressed)",
                left.chars().count(),
                right.chars().count()
            )
        }),
        _ => (left != right).then(|| {
            format!(
                "{path}: value mismatch: {} vs {}",
                value_summary(left),
                value_summary(right)
            )
        }),
    }
}

pub fn compare_persistence(
    source: &NextArtifact,
    hydrated: &NextArtifact,
) -> Result<PersistenceComparison, PersistenceComparisonError> {
    if source == hydrated {
        return Ok(PersistenceComparison {
            equivalent: true,
            first_difference: None,
        });
    }

    let source = serde_json::to_value(source)?;
    let hydrated = serde_json::to_value(hydrated)?;
    let first_difference = first_difference_at(&source, &hydrated, "$");
    Ok(PersistenceComparison {
        equivalent: first_difference.is_none(),
        first_difference,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_known_one_ulp_decimal_round_trip_drift() {
        let left = json!(21.599999999999994_f64);
        let right = json!(21.59999999999999_f64);
        assert!(first_difference_at(&left, &right, "$").is_none());
    }

    #[test]
    fn rejects_two_ulp_float_change() {
        let left = 21.599999999999994_f64;
        let right = f64::from_bits(left.to_bits() - 2);
        let difference = first_difference_at(&json!(left), &json!(right), "$").unwrap();
        assert!(difference.contains("beyond 1 ULP"));
    }

    #[test]
    fn integers_remain_strict() {
        assert!(first_difference_at(&json!(42), &json!(43), "$").is_some());
    }

    #[test]
    fn diagnostics_do_not_echo_string_contents() {
        let left = json!({"name": "confidential-a"});
        let right = json!({"name": "confidential-b"});
        let difference = first_difference_at(&left, &right, "$").unwrap();
        assert!(difference.starts_with("$.name:"));
        assert!(!difference.contains("confidential-a"));
        assert!(!difference.contains("confidential-b"));
    }
}
