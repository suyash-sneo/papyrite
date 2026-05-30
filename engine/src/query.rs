use std::collections::BTreeMap;

use crate::error::{DbError, Result};
use crate::types::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateRequest {
    pub id: String,
    pub set: BTreeMap<String, Value>,
    pub unset: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FindRequest {
    pub path: String,
    pub eq: Value,
}

pub fn parse_id_filter_json(input: &str) -> Result<String> {
    parse_id_filter_value(serde_json::from_str(input)?)
}

pub fn parse_find_json(input: &str) -> Result<FindRequest> {
    parse_find_value(serde_json::from_str(input)?)
}

pub fn parse_update_json(input: &str) -> Result<UpdateRequest> {
    parse_update_value(serde_json::from_str(input)?)
}

fn parse_id_filter_value(value: serde_json::Value) -> Result<String> {
    let object = expect_object(value, "expected object filter")?;
    if object.len() != 1 {
        return Err(DbError::UnsupportedQuery(
            "only {_id: string} filters are supported".to_string(),
        ));
    }

    match object.get("_id") {
        Some(serde_json::Value::String(id)) => Ok(id.clone()),
        Some(_) => Err(DbError::InvalidData(
            "_id filter must be a string".to_string(),
        )),
        None => Err(DbError::UnsupportedQuery(
            "only {_id: string} filters are supported".to_string(),
        )),
    }
}

fn parse_find_value(value: serde_json::Value) -> Result<FindRequest> {
    let object = expect_object(value, "expected object find query")?;
    if object.len() != 2 || !object.contains_key("path") || !object.contains_key("eq") {
        return Err(DbError::UnsupportedQuery(
            "find expects {\"path\": string, \"eq\": value}".to_string(),
        ));
    }

    let path = match object.get("path") {
        Some(serde_json::Value::String(path)) if is_valid_path(path) => path.clone(),
        Some(serde_json::Value::String(_)) => {
            return Err(DbError::InvalidData("path must not be empty".to_string()));
        }
        Some(_) => return Err(DbError::InvalidData("path must be a string".to_string())),
        None => unreachable!(),
    };
    let eq = Value::from_json_value(object.get("eq").cloned().unwrap())?;

    Ok(FindRequest { path, eq })
}

fn parse_update_value(value: serde_json::Value) -> Result<UpdateRequest> {
    let mut object = expect_object(value, "expected object update request")?;

    let filter = object
        .remove("filter")
        .ok_or_else(|| DbError::UnsupportedQuery("update requires filter".to_string()))?;
    let id = parse_id_filter_value(filter)?;

    let set = match object.remove("set") {
        None => BTreeMap::new(),
        Some(serde_json::Value::Object(values)) => values
            .into_iter()
            .map(|(path, value)| {
                if !is_valid_path(&path) {
                    return Err(DbError::InvalidData(format!("invalid set path: {path}")));
                }
                Ok((path, Value::from_json_value(value)?))
            })
            .collect::<Result<BTreeMap<_, _>>>()?,
        Some(_) => {
            return Err(DbError::InvalidData(
                "update set must be an object".to_string(),
            ));
        }
    };

    let unset = match object.remove("unset") {
        None => Vec::new(),
        Some(serde_json::Value::Array(values)) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(path) if is_valid_path(&path) => Ok(path),
                serde_json::Value::String(path) => {
                    Err(DbError::InvalidData(format!("invalid unset path: {path}")))
                }
                _ => Err(DbError::InvalidData(
                    "update unset entries must be strings".to_string(),
                )),
            })
            .collect::<Result<Vec<_>>>()?,
        Some(_) => {
            return Err(DbError::InvalidData(
                "update unset must be an array of strings".to_string(),
            ));
        }
    };

    if !object.is_empty() {
        return Err(DbError::UnsupportedQuery(
            "update contains unsupported keys".to_string(),
        ));
    }

    Ok(UpdateRequest { id, set, unset })
}

fn expect_object(
    value: serde_json::Value,
    context: &str,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    match value {
        serde_json::Value::Object(map) => Ok(map),
        _ => Err(DbError::UnsupportedQuery(context.to_string())),
    }
}

fn is_valid_path(path: &str) -> bool {
    !path.is_empty() && path.split('.').all(|segment| !segment.is_empty())
}
