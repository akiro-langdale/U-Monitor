//! RAM view helpers shared with the live telemetry panel.
use u_monitor_core::{PlatformResult, RamInfo};

pub(crate) mod history;

pub(crate) fn usage(info: &RamInfo) -> Option<f32> {
    match (&info.total_bytes, &info.used_bytes) {
        (PlatformResult::Success(t), PlatformResult::Success(u))
            if t.value > 0 && u.value <= t.value =>
        {
            Some((u.value as f64 / t.value as f64) as f32)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{DataSource, Measurement};

    fn field(value: u64) -> PlatformResult<u64> {
        PlatformResult::Success(Measurement {
            value,
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }

    #[test]
    fn progress_requires_valid_metrics() {
        let mut info = RamInfo {
            total_bytes: field(10),
            available_bytes: field(5),
            used_bytes: field(5),
        };
        assert_eq!(usage(&info), Some(0.5));
        for (total, used) in [(0, 0), (10, 11)] {
            info.total_bytes = field(total);
            info.used_bytes = field(used);
            assert!(usage(&info).is_none());
        }
        info.used_bytes = PlatformResult::Unavailable("missing".into());
        assert!(usage(&info).is_none());
    }
}
