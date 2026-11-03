use core::sync::atomic::Ordering;

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::{
    interrupt::typelevel::Binding,
    peripherals::USB,
    usb::{self, DmPin, DpPin, Driver, Instance},
    Peri,
};
use embassy_sync::channel::Channel;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Timer;
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    UsbDevice,
};
use static_cell::StaticCell;

use postcard::to_slice_cobs;
use protocol::{Calibration, DeviceMsg, Sample, SampleBlock};

use crate::stream;
use crate::stream::next;
use crate::{current_sense, STEP};

static CALIBRATION: Signal<CriticalSectionRawMutex, Calibration> = Signal::new();

/// Hands the startup calibration to the USB task, which sends it on every connect.
pub fn set_calibration(cal: Calibration) {
    CALIBRATION.signal(cal);
}

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

    let class = {
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
#[embassy_executor::task]
async fn tx_task(mut class: CdcAcmClass<'static, UsbDriver>) {
    let cal = CALIBRATION.wait().await;
    loop {
        class.wait_connection().await;
        // Every new host needs the calibration first

        while !class.dtr() {
            Timer::after_millis(10).await;
        }
        stream::clear(); // drop stale blocks from before the connect
        if send(&mut class, &DeviceMsg::Calibration(cal))
            .await
            .is_err()
        {
            continue;
        }

        loop {
            if !class.dtr() {
                break;
            }
            let sample = next().await;
            let msg = DeviceMsg::CurrentSamples(sample);
            if send(&mut class, &msg).await.is_err() {
                info!("error");
                break;
            }
        }
    }
}

async fn send(
    class: &mut CdcAcmClass<'static, UsbDriver>,
    msg: &DeviceMsg,
) -> Result<(), EndpointError> {
    let mut buf = [0; 256];
    let frame = to_slice_cobs(msg, &mut buf).unwrap();
    for chunk in frame.chunks(64) {
        class.write_packet(chunk).await?;
    }
    Ok(())
}
