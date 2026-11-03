//! CPU discovery data, independent of any OS library.
use crate::PlatformResult;
use std::num::NonZeroUsize;

/// OS-reported CPU information, possibly limited by a VM or container.
/// Fields have separate outcomes so missing topology never hides a model.
/// Multiple distinct model names are retained on heterogeneous/multi-socket hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuInfo {
    pub model_names: PlatformResult<Vec<String>>,
    pub logical_processors: PlatformResult<NonZeroUsize>,
    pub physical_cores: PlatformResult<NonZeroUsize>,
}
