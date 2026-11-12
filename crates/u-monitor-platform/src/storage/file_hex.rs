//! Adapts U-Library file samples to the monitor's measurement types.

use std::{io::ErrorKind, path::Path};
use u_library_core::ReadError;
use u_library_memory::FileSource;
use u_monitor_core::{
    DataSource, DiagnosticError, FileBytes, FileReadRequest, Measurement, PlatformResult,
};

/// Upper bound for one file HEX sample returned to the inspector.
pub const MAX_FILE_SAMPLE_BYTES: usize = 512;

pub(crate) fn read_file_bytes(request: FileReadRequest) -> PlatformResult<FileBytes> {
    let length = request.length.min(MAX_FILE_SAMPLE_BYTES);
    if length == 0 {
        return PlatformResult::Unavailable("File sample length must be greater than zero".into());
    }

    let mut file = match FileSource::open(&request.path) {
        Ok(file) => file,
        Err(error) => return file_error("open", &request.path, error),
    };
    let mut bytes = vec![0; length];
    let receipt = match file.read_at(request.offset, &mut bytes) {
        Ok(receipt) => receipt,
        Err(error) => return file_error("read", &request.path, error),
    };
    bytes.truncate(receipt.bytes_read);

    PlatformResult::Success(Measurement {
        value: FileBytes {
            path: request.path,
            offset: receipt.address,
            bytes,
        },
        source: DataSource::Measured,
        timestamp: receipt.captured_at.into(),
    })
}

fn file_error(operation: &str, path: &Path, error: ReadError) -> PlatformResult<FileBytes> {
    let message = format!("{operation}({}): {error}", path.display());
    match error {
        ReadError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            PlatformResult::PermissionRequired(message)
        }
        ReadError::Io(error) if error.kind() == ErrorKind::NotFound => {
            PlatformResult::Unavailable(message)
        }
        ReadError::UnsupportedSource => PlatformResult::Unavailable(message),
        _ => PlatformResult::Error(DiagnosticError::PlatformError(message)),
    }
}
