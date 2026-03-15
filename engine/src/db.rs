use std::fs::{File, OpenOptions};
use std::io::{self, BufReader};

use crate::record::Record;

pub struct Database {
    path: String,
}

impl Database {

    pub fn open(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
    
    pub fn put(&self, key: &str, value: &str) -> io::Result<()> {

        let mut file = OpenOptions::new() 
            .create(true)
            .append(true)
            .open(&self.path)?;

        let record = Record {
            key: key.to_string(),
            value: value.to_string(),
        };

        record.write(&mut file)
    }

    pub fn get(&self, key: &str) -> io::Result<Option<String>> {

        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };

        let mut reader = BufReader::new(file);
        let mut result = None;

        while let Some(record) = Record::read(&mut reader)? {
            if record.key == key {
                result = Some(record.value);
            }
        }

        Ok(result)
    }

    pub fn dump(&self) -> io::Result<()> {

        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        while let Some(record) = Record::read(&mut reader)? {
            println!("{} = {}", record.key, record.value);
        }

        Ok(())
    }
}
