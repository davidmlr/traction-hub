use crate::fmt::info;
use crate::system::{
    event::Events, event::EVENT_CHANNEL, state::STATE_CHANGED, state::SYSTEM_STATE,
};

#[embassy_executor::task]
pub async fn orchestrate() {
    let receiver = EVENT_CHANNEL.receiver();

    loop {
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
