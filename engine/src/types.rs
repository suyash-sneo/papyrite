use std::collections::BTreeMap;

use crate::error::{DbError, Result};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Blob(Vec<u8>),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn from_json_str(input: &str) -> Result<Self> {
        Self::from_json_value(serde_json::from_str(input)?)
    }

    pub fn from_json_value(value: serde_json::Value) -> Result<Self> {
        match value {
            serde_json::Value::Null => Ok(Self::Null),
            serde_json::Value::Bool(value) => Ok(Self::Bool(value)),
            serde_json::Value::Number(value) => {
                if let Some(int) = value.as_i64() {
                    Ok(Self::Int(int))
                } else if let Some(float) = value.as_f64() {
                    Ok(Self::Float(float))
                } else {
                    Err(DbError::InvalidData(
                        "numeric value is out of range".to_string(),
                    ))
                }
            }
            serde_json::Value::String(value) => Ok(Self::String(value)),
            serde_json::Value::Array(values) => values
                .into_iter()
                .map(Self::from_json_value)
                .collect::<Result<Vec<_>>>()
                .map(Self::Array),
            serde_json::Value::Object(values) => values
                .into_iter()
                .map(|(key, value)| Ok((key, Self::from_json_value(value)?)))
                .collect::<Result<BTreeMap<_, _>>>()
                .map(Self::Object),
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Int(value) => serde_json::Value::Number((*value).into()),
            Self::Float(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Self::String(value) => serde_json::Value::String(value.clone()),
            Self::Blob(bytes) => serde_json::Value::Array(
                bytes
                    .iter()
                    .map(|byte| serde_json::Value::Number((*byte as u64).into()))
                    .collect(),
            ),
            Self::Array(values) => {
                serde_json::Value::Array(values.iter().map(Self::to_json_value).collect())
            }
            Self::Object(values) => serde_json::Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_json_value()))
                    .collect(),
            ),
        }
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.to_json_value()).map_err(DbError::from)
    }

    pub fn get_path(&self, path: &str) -> Option<&Value> {
        if path.is_empty() {
            return Some(self);
        }

        let mut current = self;
        for segment in path.split('.') {
            match current {
                Self::Object(map) => current = map.get(segment)?,
                _ => return None,
            }
        }

        Some(current)
    }

    pub fn get_path_mut(&mut self, path: &str) -> Option<&mut Value> {
        if path.is_empty() {
            return Some(self);
        }

        let (head, tail) = path
            .split_once('.')
            .map_or((path, ""), |(head, tail)| (head, tail));
        match self {
            Self::Object(map) => {
                let value = map.get_mut(head)?;
                if tail.is_empty() {
                    Some(value)
                } else {
                    value.get_path_mut(tail)
                }
            }
            _ => None,
        }
    }

    pub fn id_str(&self) -> Option<&str> {
        match self {
            Self::Object(map) => match map.get("_id") {
                Some(Self::String(id)) => Some(id.as_str()),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn json_object_round_trip() {
        let value = Value::from_json_str(r#"{"_id":"u1","name":"Anna","age":30}"#).unwrap();
        let round_trip = Value::from_json_value(value.to_json_value()).unwrap();
        assert_eq!(value, round_trip);
    }

    #[test]
    fn nested_arrays_and_objects_round_trip() {
        let value = Value::from_json_str(
            r#"{"_id":"u1","profile":{"tags":["a",{"nested":true}],"score":1.5}}"#,
        )
        .unwrap();
        let json = value.to_pretty_json().unwrap();
        let reparsed = Value::from_json_str(&json).unwrap();
        assert_eq!(value, reparsed);
    }

    #[test]
    fn get_path_reads_nested_field() {
        let value = Value::from_json_str(
            r#"{"_id":"u1","profile":{"email":"a@example.com","flags":{"active":true}}}"#,
        )
        .unwrap();

        assert_eq!(
            value.get_path("profile.email"),
            Some(&Value::String("a@example.com".to_string()))
        );
        assert_eq!(
            value.get_path("profile.flags.active"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn get_path_missing_returns_none() {
        let value =
            Value::from_json_str(r#"{"_id":"u1","profile":{"email":"a@example.com"}}"#).unwrap();
        assert_eq!(value.get_path("profile.phone"), None);
        assert_eq!(value.get_path("profile.email.domain"), None);
    }
}
