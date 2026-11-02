use super::CommandError;
use crate::output::Format;
use serde::Serialize;
use std::io::{self, Write};
pub(super) fn error(message: String) -> CommandError {
    CommandError::new(
        if message.contains("stale") {
            "stale_index"
        } else {
            "index_unavailable"
        },
        message,
    )
}
pub(super) fn emit(
    command: &str,
    data: &impl Serialize,
    incomplete: bool,
    format: Format,
    human: String,
) -> Result<(), CommandError> {
    let bytes = match format {
        Format::Json => {
            let mut b=serde_json::to_vec_pretty(&serde_json::json!({"format_version":1,"command":command,"incomplete_analysis":incomplete,"data":data})).map_err(|e|error(e.to_string()))?;
            b.push(b'\n');
            b
        }
        Format::Human => human.into_bytes(),
    };
    io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|e| error(e.to_string()))?;
    if incomplete {
        writeln!(io::stderr().lock(),"warning: incomplete static analysis; unknown boundaries and unsupported facts remain visible").map_err(|e|error(e.to_string()))?;
    }
    Ok(())
}
