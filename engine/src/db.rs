use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use crate::codec::{decode_value, encode_value};
use crate::error::{DbError, Result};
use crate::query::{UpdateRequest, parse_find_json, parse_id_filter_json, parse_update_json};
use crate::record::Record;
use crate::types::Value;

pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn create_json(&self, json: &str) -> Result<()> {
        self.create(Value::from_json_str(json)?)
    }

    pub fn get_json(&self, json: &str) -> Result<Option<String>> {
        let id = parse_id_filter_json(json)?;
        self.get_by_id(&id)?
            .map(|value| value.to_pretty_json())
            .transpose()
    }

    pub fn delete_json(&self, json: &str) -> Result<bool> {
        let id = parse_id_filter_json(json)?;
        self.delete_by_id(&id)
    }

    pub fn update_json(&self, json: &str) -> Result<()> {
        self.update(parse_update_json(json)?)
    }

    pub fn find_json(&self, json: &str) -> Result<String> {
        let request = parse_find_json(json)?;
        let matches = self.find_eq(&request.path, &request.eq)?;
        pretty_json_array(&matches)
    }

    pub fn dump_json(&self) -> Result<String> {
        pretty_json_array(&self.dump()?)
    }

    pub fn create(&self, doc: Value) -> Result<()> {
        let id = validate_document(&doc)?.to_string();
        if self.get_by_id(&id)?.is_some() {
            return Err(DbError::DuplicateId(id));
        }

        self.append_record(Record::Put {
            id,
            doc: encode_value(&doc)?,
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Value>> {
        let mut latest = None;

        self.scan_records(|record| {
            match record {
                Record::Put { id: record_id, doc } if record_id == id => {
                    latest = Some(Some(decode_document(&record_id, &doc)?));
                }
                Record::Delete { id: record_id } if record_id == id => {
                    latest = Some(None);
                }
                _ => {}
            }
            Ok(())
        })?;

        Ok(latest.flatten())
    }

    pub fn delete_by_id(&self, id: &str) -> Result<bool> {
        if self.get_by_id(id)?.is_none() {
            return Ok(false);
        }

        self.append_record(Record::Delete { id: id.to_string() })?;
        Ok(true)
    }

    pub fn update(&self, update: UpdateRequest) -> Result<()> {
        let mut doc = self
            .get_by_id(&update.id)?
            .ok_or_else(|| DbError::NotFound(update.id.clone()))?;
        let current_id = validate_document(&doc)?.to_string();

        for path in update.set.keys() {
            if path == "_id" {
                match update.set.get(path) {
                    Some(Value::String(id)) if id == &current_id => {}
                    Some(_) => {
                        return Err(DbError::InvalidData(
                            "updating _id to a different value is not allowed".to_string(),
                        ));
                    }
                    None => unreachable!(),
                }
            }
        }

        for path in &update.unset {
            if path == "_id" {
                return Err(DbError::InvalidData(
                    "unsetting _id is not allowed".to_string(),
                ));
            }
        }

        for (path, value) in update.set {
            set_path(&mut doc, &path, value)?;
        }

        for path in update.unset {
            unset_path(&mut doc, &path)?;
        }

        let final_id = validate_document(&doc)?;
        if final_id != current_id {
            return Err(DbError::InvalidData(
                "document _id changed during update".to_string(),
            ));
        }

        self.append_record(Record::Put {
            id: current_id,
            doc: encode_value(&doc)?,
        })
    }

    pub fn find_eq(&self, path: &str, needle: &Value) -> Result<Vec<Value>> {
        if !is_valid_path(path) {
            return Err(DbError::InvalidData("path must not be empty".to_string()));
        }

        Ok(self
            .live_documents()?
            .into_values()
            .filter(|doc| doc.get_path(path) == Some(needle))
            .collect())
    }

    pub fn dump(&self) -> Result<Vec<Value>> {
        Ok(self.live_documents()?.into_values().collect())
    }

    fn append_record(&self, record: Record) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        record.write_to(&mut file)?;
        file.flush()?;
        Ok(())
    }

    fn live_documents(&self) -> Result<BTreeMap<String, Value>> {
        let mut state: BTreeMap<String, Option<Value>> = BTreeMap::new();

        self.scan_records(|record| {
            match record {
                Record::Put { id, doc } => {
                    let doc = decode_document(&id, &doc)?;
                    state.insert(id, Some(doc));
                }
                Record::Delete { id } => {
                    state.insert(id, None);
                }
            }
            Ok(())
        })?;

        Ok(state
            .into_iter()
            .filter_map(|(id, doc)| doc.map(|doc| (id, doc)))
            .collect())
    }

    fn scan_records<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(Record) -> Result<()>,
    {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(DbError::Io(err)),
        };

        let mut reader = BufReader::new(file);
        while let Some(record) = Record::read_from(&mut reader)? {
            f(record)?;
        }

        Ok(())
    }
}

pub fn set_path(root: &mut Value, path: &str, value: Value) -> Result<()> {
    let segments = split_path(path)?;
    set_path_segments(root, &segments, value)
}

pub fn unset_path(root: &mut Value, path: &str) -> Result<()> {
    let segments = split_path(path)?;
    unset_path_segments(root, &segments)
}

fn set_path_segments(root: &mut Value, segments: &[&str], value: Value) -> Result<()> {
    match root {
        Value::Object(map) => {
            if segments.len() == 1 {
                map.insert(segments[0].to_string(), value);
                return Ok(());
            }

            let entry = map
                .entry(segments[0].to_string())
                .or_insert_with(|| Value::Object(BTreeMap::new()));
            match entry {
                Value::Object(_) => set_path_segments(entry, &segments[1..], value),
                _ => Err(DbError::TypeMismatch(format!(
                    "path segment '{}' is not an object",
                    segments[0]
                ))),
            }
        }
        _ => Err(DbError::TypeMismatch(
            "root document must be an object".to_string(),
        )),
    }
}

fn unset_path_segments(root: &mut Value, segments: &[&str]) -> Result<()> {
    match root {
        Value::Object(map) => {
            if segments.len() == 1 {
                map.remove(segments[0]);
                return Ok(());
            }

            match map.get_mut(segments[0]) {
                Some(Value::Object(_)) => {
                    unset_path_segments(map.get_mut(segments[0]).unwrap(), &segments[1..])
                }
                Some(_) | None => Ok(()),
            }
        }
        _ => Err(DbError::TypeMismatch(
            "root document must be an object".to_string(),
        )),
    }
}

fn split_path(path: &str) -> Result<Vec<&str>> {
    if !is_valid_path(path) {
        return Err(DbError::InvalidData(format!("invalid path: {path}")));
    }
    Ok(path.split('.').collect())
}

fn is_valid_path(path: &str) -> bool {
    !path.is_empty() && path.split('.').all(|segment| !segment.is_empty())
}

fn decode_document(id: &str, bytes: &[u8]) -> Result<Value> {
    let (value, used) = decode_value(bytes)?;
    if used != bytes.len() {
        return Err(DbError::InvalidFormat(
            "document payload contains trailing bytes".to_string(),
        ));
    }
    if !matches!(value, Value::Object(_)) {
        return Err(DbError::InvalidRootDocument);
    }
    let doc_id = validate_document(&value)?;
    if doc_id != id {
        return Err(DbError::InvalidData(format!(
            "record id '{id}' does not match document _id '{doc_id}'"
        )));
    }
    Ok(value)
}

fn validate_document(doc: &Value) -> Result<&str> {
    match doc {
        Value::Object(map) => match map.get("_id") {
            Some(Value::String(id)) => Ok(id.as_str()),
            Some(_) => Err(DbError::InvalidData(
                "document _id must be a string".to_string(),
            )),
            None => Err(DbError::MissingId),
        },
        _ => Err(DbError::InvalidRootDocument),
    }
}

fn pretty_json_array(values: &[Value]) -> Result<String> {
    let json = serde_json::Value::Array(values.iter().map(Value::to_json_value).collect());
    serde_json::to_string_pretty(&json).map_err(DbError::from)
}
