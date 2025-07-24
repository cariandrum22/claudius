#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Replace all existing MCP servers with new ones
    Replace,
    /// Merge new servers, overwriting existing ones with same name
    Merge,
    /// Merge new servers, preserving existing ones with same name
    MergePreserveExisting,
    /// Merge with interactive conflict resolution
    InteractiveMerge,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::InteractiveMerge
    }
}
