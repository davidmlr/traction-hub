use embassy_stm32::adc::{Adc, SampleTime};
use embassy_time::{Duration, Timer};

use crate::system::{
    event::{Events, EVENT_CHANNEL},
    resources::Voltage,
};

#[embassy_executor::task]
pub async fn voltage(mut r: Voltage) {
    let sender = EVENT_CHANNEL.sender();

    let mut adc = Adc::new(r.adc);
    adc.set_resolution(embassy_stm32::adc::Resolution::BITS12);
    adc.set_sample_time(SampleTime::CYCLES24_5);

    loop {
        Timer::after(Duration::from_secs(1)).await;
        let meas_vcc_adc: f32 = adc.blocking_read(&mut r.meas_pin).into();
        let meas_vcc = 3.3 / 4095.0 * meas_vcc_adc * 5.7;

        sender.send(Events::VsysVoltage(meas_vcc)).await;
    }
}
