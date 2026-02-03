use embassy_executor::Spawner;
use embassy_stm32::adc::AdcConfig;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_time::{Duration, Timer};

use crate::system::{
    event::{Events, EVENT_CHANNEL},
    resources::Voltage,
};

#[embassy_executor::task]
pub async fn voltage(_spawner: Spawner, mut r: Voltage) {
    let sender = EVENT_CHANNEL.sender();

    let mut adc_config = AdcConfig::default();
    adc_config.resolution = Some(embassy_stm32::adc::Resolution::BITS12);
    let mut adc = Adc::new(r.adc, adc_config);

    loop {
        Timer::after(Duration::from_secs(1)).await;
        let meas_vcc_adc: f32 = adc
            .blocking_read(&mut r.meas_pin, SampleTime::CYCLES24_5)
            .into();
        let meas_vcc = 3.3 / 4095.0 * meas_vcc_adc * 5.7;

        sender.send(Events::VsysVoltage(meas_vcc)).await;
    }
}
