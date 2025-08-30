use embassy_stm32::gpio::{Input, Pull};
use embassy_time::{Duration, Timer};

use crate::system::{
    event::{Events, EVENT_CHANNEL},
    resources::Voltage,
};

#[embassy_executor::task]
pub async fn voltage(r: Voltage) {
    let voltage_pin = Input::new(r.meas_pin, Pull::None);
    let sender = EVENT_CHANNEL.sender();

    loop {
        Timer::after(Duration::from_secs(1)).await;
        let mut voltage = 10.0;
        if voltage_pin.is_low() {
            voltage = 5.0;
        }
        sender.send(Events::VsysVoltage(voltage)).await;
    }
}
