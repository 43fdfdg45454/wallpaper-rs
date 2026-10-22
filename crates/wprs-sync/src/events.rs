use crate::{Error, Result};
use serde_json::Value;

const LIMIT: usize = 65_536;

#[derive(Default)]
pub struct Parser {
    line: Vec<u8>,
    event: String,
    data: String,
}

impl Parser {
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<(String, Value)>> {
        let mut out = Vec::new();
        for &byte in chunk {
            if byte != b'\n' {
                self.line.push(byte);
                if self.line.len() > LIMIT {
                    return Err(Error::Invalid("the server sent an event line over 64 KiB".into()));
                }
                continue;
            }
            let line = String::from_utf8_lossy(&self.line).trim_end_matches('\r').to_owned();
            self.line.clear();
            match line.split_once(':') {
                None if line.is_empty() => {
                    let (event, data) = (std::mem::take(&mut self.event), std::mem::take(&mut self.data));
                    if let Ok(value) = serde_json::from_str(&data) {
                        out.push((if event.is_empty() { "message".into() } else { event }, value));
                    }
                }
                Some(("event", name)) => self.event = name.trim().into(),
                Some(("data", data)) => {
                    self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
                    if self.data.len() > LIMIT {
                        return Err(Error::Invalid("the server sent an event over 64 KiB".into()));
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }
}
