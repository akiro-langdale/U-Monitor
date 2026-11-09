//! Read-only capped samples from user-selected regular files.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};
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

    let metadata = match std::fs::metadata(&request.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return PlatformResult::PermissionRequired(format!(
                "metadata({}) denied: {error}",
                request.path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PlatformResult::Unavailable(format!(
                "file not found: {}",
                request.path.display()
            ));
        }
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "metadata({}) failed: {error}",
                request.path.display()
            )));
        }
    };

    if !metadata.is_file() {
        return PlatformResult::Unavailable(format!(
            "{} is not a regular file; raw devices are not opened for hex samples",
            request.path.display()
        ));
    }

    let mut file = match File::open(&request.path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return PlatformResult::PermissionRequired(format!(
                "open({}) denied: {error}",
                request.path.display()
            ));
        }
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "open({}) failed: {error}",
                request.path.display()
            )));
        }
    };

    if let Err(error) = file.seek(SeekFrom::Start(request.offset)) {
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "seek({}) failed: {error}",
            request.path.display()
        )));
    }

    let mut bytes = vec![0_u8; length];
    let read = match file.read(&mut bytes) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return PlatformResult::PermissionRequired(format!(
                "read({}) denied: {error}",
                request.path.display()
            ));
        }
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "read({}) failed: {error}",
                request.path.display()
            )));
        }
    };
    bytes.truncate(read);

    PlatformResult::Success(Measurement {
        value: FileBytes {
            path: request.path,
            offset: request.offset,
            bytes,
        },
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}
