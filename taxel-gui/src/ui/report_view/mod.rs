use std::time::Duration;

pub mod navigation;
pub mod search_overlay;
pub mod sidebar;
pub mod table;
pub mod toolbar;

/// Duration for which to keep the "jump highlight" active after navigating to a
/// fact row.
const JUMP_HIGHLIGHT_DURATION: Duration = Duration::from_millis(1400);
