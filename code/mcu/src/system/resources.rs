use assign_resources::assign_resources;
use embassy_stm32::peripherals;
use embassy_stm32::bind_interrupts;
use embassy_stm32::usb;
use embassy_stm32::Peri;

assign_resources! {
    voltage: Voltage {
        meas_pin: PA6,
        adc: ADC2,
    }
    usb: Usb {
        usb: USB,
        dp: PA12,
        dm: PA11,
    }
}

bind_interrupts!(pub struct Irqs {
    USB_LP => usb::InterruptHandler<peripherals::USB>;
});
