use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

use crate::error::{Result, XurlError};

pub fn parse_json_line(path: &Path, line_no: usize, line: &str) -> Result<Option<Value>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value =
        serde_json::from_str::<Value>(trimmed).map_err(|source| XurlError::InvalidJsonLine {
            path: path.to_path_buf(),
            line: line_no,
            source,
        })?;
    Ok(Some(value))
}

pub fn parse_jsonl_reader<R, F>(path: &Path, mut reader: R, mut on_value: F) -> Result<()>
where
    R: BufRead,
    F: FnMut(usize, Value) -> Result<()>,
{
    let mut line_no = 0usize;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|source| XurlError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if bytes == 0 {
            break;
        }

        line_no += 1;
        if let Some(value) = parse_json_line(path, line_no, &line)? {
            on_value(line_no, value)?;
        }
    }

    Ok(())
}
