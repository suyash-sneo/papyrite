use std::collections::BTreeMap;

use crate::error::{DbError, Result};
use crate::types::Value;

const TAG_NULL: u8 = 0;
const TAG_FALSE: u8 = 1;
const TAG_TRUE: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_FLOAT: u8 = 4;
const TAG_STRING: u8 = 5;
const TAG_BLOB: u8 = 6;
const TAG_ARRAY: u8 = 7;
const TAG_OBJECT: u8 = 8;

pub fn encode_value(value: &Value) -> Result<Vec<u8>> {
    match value {
        Value::Null => Ok(vec![TAG_NULL]),
        Value::Bool(false) => Ok(vec![TAG_FALSE]),
        Value::Bool(true) => Ok(vec![TAG_TRUE]),
        Value::Int(value) => {
            let mut bytes = vec![TAG_INT];
            bytes.extend_from_slice(&value.to_le_bytes());
            Ok(bytes)
        }
        Value::Float(value) => {
            let mut bytes = vec![TAG_FLOAT];
            bytes.extend_from_slice(&value.to_le_bytes());
            Ok(bytes)
        }
        Value::String(value) => {
            let data = value.as_bytes();
            let len = u32::try_from(data.len())
                .map_err(|_| DbError::InvalidData("string is too large to encode".to_string()))?;
            let mut bytes = vec![TAG_STRING];
            bytes.extend_from_slice(&len.to_le_bytes());
            bytes.extend_from_slice(data);
            Ok(bytes)
        }
        Value::Blob(value) => {
            let len = u32::try_from(value.len())
                .map_err(|_| DbError::InvalidData("blob is too large to encode".to_string()))?;
            let mut bytes = vec![TAG_BLOB];
            bytes.extend_from_slice(&len.to_le_bytes());
            bytes.extend_from_slice(value);
            Ok(bytes)
        }
        Value::Array(values) => {
            let mut payload = Vec::new();
            for value in values {
                payload.extend_from_slice(&encode_value(value)?);
            }

            let elem_count = u32::try_from(values.len())
                .map_err(|_| DbError::InvalidData("array has too many elements".to_string()))?;
            let arr_len = u64::try_from(payload.len())
                .map_err(|_| DbError::InvalidData("array payload is too large".to_string()))?;

            let mut bytes = vec![TAG_ARRAY];
            bytes.extend_from_slice(&arr_len.to_le_bytes());
            bytes.extend_from_slice(&elem_count.to_le_bytes());
            bytes.extend_from_slice(&payload);
            Ok(bytes)
        }
        Value::Object(fields) => encode_object(fields),
    }
}

pub fn decode_value(bytes: &[u8]) -> Result<(Value, usize)> {
    let tag = *bytes
        .first()
        .ok_or_else(|| DbError::InvalidFormat("missing value tag".to_string()))?;

    match tag {
        TAG_NULL => Ok((Value::Null, 1)),
        TAG_FALSE => Ok((Value::Bool(false), 1)),
        TAG_TRUE => Ok((Value::Bool(true), 1)),
        TAG_INT => {
            let end = 1 + 8;
            require_len(bytes, end, "truncated int payload")?;
            let value = i64::from_le_bytes(bytes[1..end].try_into().unwrap());
            Ok((Value::Int(value), end))
        }
        TAG_FLOAT => {
            let end = 1 + 8;
            require_len(bytes, end, "truncated float payload")?;
            let value = f64::from_le_bytes(bytes[1..end].try_into().unwrap());
            Ok((Value::Float(value), end))
        }
        TAG_STRING => decode_bytes(bytes, TAG_STRING, "string").and_then(|(data, used)| {
            let value = std::str::from_utf8(data)?;
            Ok((Value::String(value.to_string()), used))
        }),
        TAG_BLOB => decode_bytes(bytes, TAG_BLOB, "blob")
            .map(|(data, used)| (Value::Blob(data.to_vec()), used)),
        TAG_ARRAY => decode_array(bytes),
        TAG_OBJECT => decode_object(bytes),
        other => Err(DbError::InvalidFormat(format!(
            "unknown value tag: {other}"
        ))),
    }
}

fn encode_object(fields: &BTreeMap<String, Value>) -> Result<Vec<u8>> {
    let field_count = u32::try_from(fields.len())
        .map_err(|_| DbError::InvalidData("object has too many fields".to_string()))?;

    let mut directory = Vec::new();
    let mut values = Vec::new();

    for (key, value) in fields {
        let key_bytes = key.as_bytes();
        let key_len = u16::try_from(key_bytes.len())
            .map_err(|_| DbError::InvalidData("object key is too long".to_string()))?;
        let offset = u32::try_from(values.len())
            .map_err(|_| DbError::InvalidData("object values section is too large".to_string()))?;
        let encoded_value = encode_value(value)?;

        directory.extend_from_slice(&key_len.to_le_bytes());
        directory.extend_from_slice(key_bytes);
        directory.push(value_tag(value));
        directory.extend_from_slice(&offset.to_le_bytes());
        values.extend_from_slice(&encoded_value);
    }

    let dir_bytes_len = u32::try_from(directory.len())
        .map_err(|_| DbError::InvalidData("object directory is too large".to_string()))?;
    let obj_len = u64::try_from(directory.len() + values.len())
        .map_err(|_| DbError::InvalidData("object payload is too large".to_string()))?;

    let mut bytes = vec![TAG_OBJECT];
    bytes.extend_from_slice(&obj_len.to_le_bytes());
    bytes.extend_from_slice(&field_count.to_le_bytes());
    bytes.extend_from_slice(&dir_bytes_len.to_le_bytes());
    bytes.extend_from_slice(&directory);
    bytes.extend_from_slice(&values);
    Ok(bytes)
}

fn decode_array(bytes: &[u8]) -> Result<(Value, usize)> {
    let header_len = 1 + 8 + 4;
    require_len(bytes, header_len, "truncated array header")?;

    let arr_len = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
    let elem_count = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
    let payload_len = usize::try_from(arr_len).map_err(|_| {
        DbError::InvalidFormat("array payload length does not fit usize".to_string())
    })?;
    let total_len = header_len
        .checked_add(payload_len)
        .ok_or_else(|| DbError::InvalidFormat("array length overflow".to_string()))?;
    require_len(bytes, total_len, "truncated array payload")?;

    let payload = &bytes[header_len..total_len];
    let mut offset = 0usize;
    let mut values = Vec::with_capacity(elem_count);

    for _ in 0..elem_count {
        let (value, used) = decode_value(&payload[offset..])?;
        values.push(value);
        offset += used;
    }

    if offset != payload.len() {
        return Err(DbError::InvalidFormat(
            "array payload length does not match encoded elements".to_string(),
        ));
    }

    Ok((Value::Array(values), total_len))
}

fn decode_object(bytes: &[u8]) -> Result<(Value, usize)> {
    let header_len = 1 + 8 + 4 + 4;
    require_len(bytes, header_len, "truncated object header")?;

    let obj_len = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
    let field_count = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
    let dir_bytes_len = u32::from_le_bytes(bytes[13..17].try_into().unwrap()) as usize;
    let obj_len = usize::try_from(obj_len)
        .map_err(|_| DbError::InvalidFormat("object length does not fit usize".to_string()))?;

    if dir_bytes_len > obj_len {
        return Err(DbError::InvalidFormat(
            "object directory length exceeds payload length".to_string(),
        ));
    }

    let total_len = header_len
        .checked_add(obj_len)
        .ok_or_else(|| DbError::InvalidFormat("object length overflow".to_string()))?;
    require_len(bytes, total_len, "truncated object payload")?;

    let dir_start = header_len;
    let values_start = header_len + dir_bytes_len;
    let dir_bytes = &bytes[dir_start..values_start];
    let values_bytes = &bytes[values_start..total_len];
    let entries = parse_directory(dir_bytes, field_count)?;

    let mut expected_offset = 0usize;
    let mut object = BTreeMap::new();

    for (key, tag, offset) in entries {
        let offset = offset as usize;
        if values_bytes.is_empty() && field_count > 0 {
            return Err(DbError::InvalidFormat(
                "object values section is empty for non-empty object".to_string(),
            ));
        }
        if offset >= values_bytes.len() {
            return Err(DbError::InvalidFormat(format!(
                "object field '{key}' offset is out of bounds"
            )));
        }
        if offset != expected_offset {
            return Err(DbError::InvalidFormat(format!(
                "object field '{key}' offset is non-contiguous"
            )));
        }

        let (value, used) = decode_value(&values_bytes[offset..])?;
        if value_tag(&value) != tag {
            return Err(DbError::InvalidFormat(format!(
                "object field '{key}' directory tag does not match encoded value"
            )));
        }
        expected_offset += used;
        object.insert(key, value);
    }

    if expected_offset != values_bytes.len() {
        return Err(DbError::InvalidFormat(
            "object values section does not match directory offsets".to_string(),
        ));
    }

    Ok((Value::Object(object), total_len))
}

fn parse_directory(dir_bytes: &[u8], field_count: usize) -> Result<Vec<(String, u8, u32)>> {
    let mut offset = 0usize;
    let mut entries = Vec::with_capacity(field_count);

    for _ in 0..field_count {
        if offset + 2 > dir_bytes.len() {
            return Err(DbError::InvalidFormat(
                "truncated object directory entry".to_string(),
            ));
        }
        let key_len =
            u16::from_le_bytes(dir_bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        let key_end = offset
            .checked_add(key_len)
            .ok_or_else(|| DbError::InvalidFormat("object key length overflow".to_string()))?;
        if key_end + 1 + 4 > dir_bytes.len() {
            return Err(DbError::InvalidFormat(
                "truncated object directory key or metadata".to_string(),
            ));
        }

        let key = std::str::from_utf8(&dir_bytes[offset..key_end])?.to_string();
        offset = key_end;

        let tag = dir_bytes[offset];
        offset += 1;

        let value_offset = u32::from_le_bytes(dir_bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;

        entries.push((key, tag, value_offset));
    }

    if offset != dir_bytes.len() {
        return Err(DbError::InvalidFormat(
            "object directory length does not match entry count".to_string(),
        ));
    }

    Ok(entries)
}

fn decode_bytes<'a>(bytes: &'a [u8], tag: u8, kind: &str) -> Result<(&'a [u8], usize)> {
    let header_len = 1 + 4;
    require_len(bytes, header_len, &format!("truncated {kind} header"))?;
    if bytes[0] != tag {
        return Err(DbError::InvalidFormat(format!("expected {kind} tag")));
    }

    let len = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize;
    let total_len = header_len
        .checked_add(len)
        .ok_or_else(|| DbError::InvalidFormat(format!("{kind} length overflow")))?;
    require_len(bytes, total_len, &format!("truncated {kind} payload"))?;
    Ok((&bytes[5..total_len], total_len))
}

fn require_len(bytes: &[u8], needed: usize, message: &str) -> Result<()> {
    if bytes.len() < needed {
        Err(DbError::InvalidFormat(message.to_string()))
    } else {
        Ok(())
    }
}

fn value_tag(value: &Value) -> u8 {
    match value {
        Value::Null => TAG_NULL,
        Value::Bool(false) => TAG_FALSE,
        Value::Bool(true) => TAG_TRUE,
        Value::Int(_) => TAG_INT,
        Value::Float(_) => TAG_FLOAT,
        Value::String(_) => TAG_STRING,
        Value::Blob(_) => TAG_BLOB,
        Value::Array(_) => TAG_ARRAY,
        Value::Object(_) => TAG_OBJECT,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::error::DbError;
    use crate::types::Value;

    use super::{TAG_NULL, TAG_OBJECT, TAG_STRING, decode_value, encode_value};

    fn round_trip(value: Value) {
        let encoded = encode_value(&value).unwrap();
        let (decoded, used) = decode_value(&encoded).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
    }

    #[test]
    fn encode_decode_primitive_values() {
        round_trip(Value::Null);
        round_trip(Value::Bool(false));
        round_trip(Value::Bool(true));
        round_trip(Value::Int(-42));
        round_trip(Value::Float(3.5));
        round_trip(Value::String("hello".to_string()));
        round_trip(Value::Blob(vec![1, 2, 3, 4]));
    }

    #[test]
    fn encode_decode_nested_object() {
        let mut nested = BTreeMap::new();
        nested.insert(
            "email".to_string(),
            Value::String("a@example.com".to_string()),
        );
        nested.insert("active".to_string(), Value::Bool(true));

        let mut root = BTreeMap::new();
        root.insert("_id".to_string(), Value::String("u1".to_string()));
        root.insert("profile".to_string(), Value::Object(nested));

        round_trip(Value::Object(root));
    }

    #[test]
    fn encode_decode_nested_array() {
        round_trip(Value::Array(vec![
            Value::Int(1),
            Value::Array(vec![Value::String("x".to_string()), Value::Null]),
            Value::Bool(true),
        ]));
    }

    #[test]
    fn encode_decode_object_with_multiple_fields() {
        let value = Value::from_json_str(
            r#"{"_id":"u1","age":30,"name":"Anna","profile":{"email":"a@example.com"}}"#,
        )
        .unwrap();
        round_trip(value);
    }

    #[test]
    fn decoder_rejects_truncated_headers() {
        let err = decode_value(&[TAG_STRING]).unwrap_err();
        assert!(matches!(err, DbError::InvalidFormat(_)));
    }

    #[test]
    fn decoder_rejects_out_of_bounds_offsets() {
        let mut bytes = vec![TAG_OBJECT];
        bytes.extend_from_slice(&(9u64).to_le_bytes());
        bytes.extend_from_slice(&(1u32).to_le_bytes());
        bytes.extend_from_slice(&(8u32).to_le_bytes());
        bytes.extend_from_slice(&(1u16).to_le_bytes());
        bytes.extend_from_slice(b"a");
        bytes.push(TAG_NULL);
        bytes.extend_from_slice(&(5u32).to_le_bytes());
        bytes.push(TAG_NULL);

        let err = decode_value(&bytes).unwrap_err();
        assert!(matches!(err, DbError::InvalidFormat(_)));
    }

    #[test]
    fn root_object_encoding_round_trip() {
        let value =
            Value::from_json_str(r#"{"_id":"u1","name":"Anna","items":[1,2,{"nested":true}]}"#)
                .unwrap();
        let encoded = encode_value(&value).unwrap();
        let (decoded, used) = decode_value(&encoded).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
    }
}
