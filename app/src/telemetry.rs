use serde::{Deserialize, Serialize};

/// One telemetry frame sent to PC at ~100 Hz over USB CDC.
#[derive(Serialize, Deserialize, Clone, Copy, Default)]
pub struct Telemetry {
    /// Milliseconds since boot (wraps after ~49 days)
    pub timestamp_ms: u32,
    /// DC link voltage [V]
    pub vbus_v: f32,
    /// Phase A current [A]
    pub ia_a: f32,
    /// Phase B current [A]
    pub ib_a: f32,
    /// Phase C current [A]
    pub ic_a: f32,
}
