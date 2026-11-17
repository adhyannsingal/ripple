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
    if command != "index" {
        ripple_core::execution::check().map_err(|e| error(e.into()))?;
    }
    let bytes = match format {
        Format::Json => {
            let mut b=serde_json::to_vec_pretty(&serde_json::json!({"format_version":1,"command":command,"incomplete_analysis":incomplete,"data":data})).map_err(|e|error(e.to_string()))?;
            b.push(b'\n');
            b
        }
        Format::Human => human.into_bytes(),
    };
    if command != "index" {
        ripple_core::execution::check().map_err(|e| error(e.into()))?;
    }
    io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|e| error(e.to_string()))?;
    if incomplete {
        writeln!(io::stderr().lock(),"warning: incomplete static analysis; unknown boundaries and unsupported facts remain visible").map_err(|e|error(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_during_serialization_prevents_read_success_output() {
        let control = ripple_core::execution::Control::new(std::time::Duration::from_secs(10));
        let _scope = control.enter();
        struct CancelWhileSerializing(ripple_core::execution::Control);
        impl serde::Serialize for CancelWhileSerializing {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.cancel();
                serializer.serialize_bool(true)
            }
        }
        assert!(
            emit(
                "status",
                &CancelWhileSerializing(control),
                false,
                Format::Json,
                String::new()
            )
            .is_err()
        );
    }
}
