use embassy_executor::Spawner;
use embassy_stm32::{
    interrupt::typelevel::Binding,
    peripherals::USB,
    usb::{self, DmPin, DpPin, Driver, Instance},
    Peri,
};
use embassy_time::Timer;
use embassy_usb::{
    class::{
        self,
        cdc_acm::{CdcAcmClass, State},
    },
    UsbDevice,
};
use static_cell::StaticCell;

use postcard::{to_slice, to_slice_cobs};
use serde::{Deserialize, Serialize};

pub fn init(
    spawner: Spawner,
    usb: Peri<'static, USB>,
    irq: impl Binding<<USB as Instance>::Interrupt, usb::InterruptHandler<USB>> + 'static,
    dp: Peri<'static, impl DpPin<USB>>,
    dm: Peri<'static, impl DmPin<USB>>,
) {
    let driver = Driver::new(usb, irq, dp, dm);

    let mut config = embassy_usb::Config::new(0xdead, 0xbeef);
    config.manufacturer = Some("ShortCircuit");
    config.product = Some("traction-hub");
    config.serial_number = Some("000000");

    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    let mut class = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        CdcAcmClass::new(&mut builder, state, 64)
    };

    let usb = builder.build();

    spawner.spawn(usb_task(usb).unwrap());
    spawner.spawn(tx_task(class).unwrap());
}

type UsbDriver = Driver<'static, USB>;

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}
#[derive(Serialize, Deserialize)]
struct TestTelemetry {
    val_1: u16,
    val_2: u16,
}

#[embassy_executor::task]
async fn tx_task(mut class: CdcAcmClass<'static, UsbDriver>) {
    let mut buf = [0; 64];
    let mut counter: u16 = 0;
    loop {
        class.wait_connection().await;
        loop {
            let msg = to_slice_cobs(
                &TestTelemetry {
                    val_1: counter,
                    val_2: 1232,
                },
                &mut buf,
            )
            .unwrap();
            if class.write_packet(msg).await.is_err() {
                break;
            };
            counter += 1;
            Timer::after_millis(100).await;
        }
    }
}
