use std::io::{Read, Write};

use crate::error::{DbError, Result};

const RECORD_PUT: u8 = 1;
const RECORD_DELETE: u8 = 2;

#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    Put { id: String, doc: Vec<u8> },
    Delete { id: String },
}

impl Record {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Self::Put { id, doc } => {
                let id_len = u32::try_from(id.len()).map_err(|_| {
                    DbError::InvalidData("record id is too large to encode".to_string())
                })?;
                let doc_len = u32::try_from(doc.len()).map_err(|_| {
                    DbError::InvalidData("record document is too large to encode".to_string())
                })?;

                writer.write_all(&[RECORD_PUT])?;
                writer.write_all(&id_len.to_le_bytes())?;
                writer.write_all(&doc_len.to_le_bytes())?;
                writer.write_all(id.as_bytes())?;
                writer.write_all(doc)?;
            }
            Self::Delete { id } => {
                let id_len = u32::try_from(id.len()).map_err(|_| {
                    DbError::InvalidData("record id is too large to encode".to_string())
                })?;

                writer.write_all(&[RECORD_DELETE])?;
                writer.write_all(&id_len.to_le_bytes())?;
                writer.write_all(&0u32.to_le_bytes())?;
                writer.write_all(id.as_bytes())?;
            }
        }

        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Option<Record>> {
        let mut tag = [0u8; 1];
        if !read_exact_or_eof(reader, &mut tag)? {
            return Ok(None);
        }

        let mut id_len_buf = [0u8; 4];
        read_exact_required(reader, &mut id_len_buf, "truncated record id length")?;
        let id_len = u32::from_le_bytes(id_len_buf) as usize;

        let mut doc_len_buf = [0u8; 4];
        read_exact_required(reader, &mut doc_len_buf, "truncated record document length")?;
        let doc_len = u32::from_le_bytes(doc_len_buf) as usize;

        let mut id_bytes = vec![0u8; id_len];
        read_exact_required(reader, &mut id_bytes, "truncated record id bytes")?;
        let id = std::str::from_utf8(&id_bytes)?.to_string();

        match tag[0] {
            RECORD_PUT => {
                let mut doc = vec![0u8; doc_len];
                read_exact_required(reader, &mut doc, "truncated record document bytes")?;
                Ok(Some(Self::Put { id, doc }))
            }
            RECORD_DELETE => {
                if doc_len != 0 {
                    return Err(DbError::InvalidFormat(
                        "delete record must have zero document length".to_string(),
                    ));
                }
                Ok(Some(Self::Delete { id }))
            }
            other => Err(DbError::InvalidFormat(format!(
                "unknown record tag: {other}"
            ))),
        }
    }
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool> {
    let mut read = 0usize;
    while read < buf.len() {
        match reader.read(&mut buf[read..]) {
            Ok(0) if read == 0 => return Ok(false),
            Ok(0) => {
                return Err(DbError::InvalidFormat(
                    "truncated record at end of file".to_string(),
                ));
            }
            Ok(count) => read += count,
            Err(err) => return Err(DbError::Io(err)),
        }
    }

    Ok(true)
}

fn read_exact_required<R: Read>(reader: &mut R, buf: &mut [u8], context: &str) -> Result<()> {
    match read_exact_or_eof(reader, buf) {
        Ok(true) => Ok(()),
        Ok(false) => Err(DbError::InvalidFormat(context.to_string())),
        Err(DbError::InvalidFormat(_)) => Err(DbError::InvalidFormat(context.to_string())),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::error::DbError;

    use super::Record;

    #[test]
    fn write_read_put_record() {
        let record = Record::Put {
            id: "u1".to_string(),
            doc: vec![1, 2, 3],
        };

        let mut bytes = Vec::new();
        record.write_to(&mut bytes).unwrap();

        let decoded = Record::read_from(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(decoded, Some(record));
    }

    #[test]
    fn write_read_delete_record() {
        let record = Record::Delete {
            id: "u1".to_string(),
        };

        let mut bytes = Vec::new();
        record.write_to(&mut bytes).unwrap();

        let decoded = Record::read_from(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(decoded, Some(record));
    }

    #[test]
    fn truncated_record_returns_error() {
        let bytes = vec![1, 1, 0, 0];
        let err = Record::read_from(&mut Cursor::new(bytes)).unwrap_err();
        assert!(matches!(err, DbError::InvalidFormat(_)));
    }

    #[test]
    fn unknown_record_tag_returns_error() {
        let bytes = vec![9, 0, 0, 0, 0, 0, 0, 0, 0];
        let err = Record::read_from(&mut Cursor::new(bytes)).unwrap_err();
        assert!(matches!(err, DbError::InvalidFormat(_)));
    }
}
