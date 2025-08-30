use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};

pub static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, Events, 10> = Channel::new();

#[derive(Debug, Clone)]
pub enum Events {
    VsysVoltage(f32), // New voltage reading
}
