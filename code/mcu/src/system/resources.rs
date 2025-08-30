use assign_resources::assign_resources;
use embassy_stm32::peripherals;
use embassy_stm32::Peri;

assign_resources! {
    voltage: Voltage {
        meas_pin: PA6,
        adc: ADC2,
    }
}
