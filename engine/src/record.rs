use std::io::{self, Read, Write};

pub struct Record {
    pub key: String,
    pub value: String,
}

impl Record {

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        
        let key_bytes = self.key.as_bytes();
        let val_bytes = self.value.as_bytes();

        let key_len = key_bytes.len() as u64;
        let val_len = val_bytes.len() as u64;

        writer.write_all(&key_len.to_le_bytes())?;
        writer.write_all(&val_len.to_le_bytes())?;

        writer.write_all(key_bytes)?;
        writer.write_all(val_bytes)?;

        Ok(())
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Option<Record>> {

        let mut key_len_buf = [0u8; 8];
        if reader.read_exact(&mut key_len_buf).is_err() {
            return Ok(None)
        }
        let key_len = u64::from_le_bytes(key_len_buf);

        let mut value_len_buf = [0u8; 8];
        reader.read_exact(&mut value_len_buf)?;
        let value_len = u64::from_le_bytes(value_len_buf);

        let mut key = vec![0u8; key_len as usize];
        let mut value = vec![0u8; value_len as usize];

        reader.read_exact(&mut key)?;
        reader.read_exact(&mut value)?;

        Ok(Some(Record {
            key: String::from_utf8_lossy(&key).to_string(),
            value: String::from_utf8_lossy(&value).to_string(),
        }))
    }
}
