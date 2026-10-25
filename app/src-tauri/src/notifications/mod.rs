//! The notifications hub (M4, D-011): Windows notifications read with the user's permission,
//! stored in `kairos.db`, shown in the panel, and summarised on request.

// The collector's poll side is used by the Windows listener (Step 4, waiting for approval).
#[allow(dead_code)]
pub mod collect;
pub mod store;

use collect::Access;

pub const ENABLED_SETTING: &str = "notifications_enabled";
pub const SINCE_SETTING: &str = "notifications_since";

/// Whether Windows lets Kairos read notifications. The listener arrives in M4 Step 4, after the
/// Step 1 probe and the `windows` crate are approved; until then Kairos can't read them anywhere.
pub fn access() -> Access {
    Access::Unavailable
}
