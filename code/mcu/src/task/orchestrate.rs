use crate::system::{
    event::EVENT_CHANNEL,
    event::Events,
    state::SYSTEM_STATE,
    state::STATE_CHANGED,
};
use defmt::info;

#[embassy_executor::task]
pub async fn orchestrate() {
    let receiver = EVENT_CHANNEL.receiver();

    loop {
        // Do nothing until we receive any event
        let event = receiver.receive().await;

        {
            let mut state = SYSTEM_STATE.lock().await;

            match event {
                Events::VsysVoltage(voltage) => {
                    state.vsys_voltage = voltage;
                    info!("Vsys voltage: {}", voltage);
                }
            }

        }

        STATE_CHANGED.signal(());
    }
}
