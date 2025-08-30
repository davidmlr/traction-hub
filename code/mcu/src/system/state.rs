use defmt::Format;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal};

pub static SYSTEM_STATE: Mutex<CriticalSectionRawMutex, SystemState> = Mutex::new(SystemState::new());
pub static STATE_CHANGED: signal::Signal<CriticalSectionRawMutex, ()> = signal::Signal::new();

/// The central state of our system, shared between tasks.
#[derive(Clone, Format)]
pub struct SystemState {
    pub vsys_voltage: f32,
}

impl SystemState {
    const fn new() -> Self {
        Self { vsys_voltage: 0.0 }
    }
}
