use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::{
    interrupt::typelevel::Binding,
    peripherals::USB,
    usb::{self, DmPin, DpPin, Driver, Instance},
    Peri,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Ticker};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, ControlChanged, Receiver, Sender, State},
    driver::EndpointError,
    Handler, UsbDevice,
};
use static_cell::StaticCell;

use postcard::{from_bytes_cobs, to_slice_cobs};
use protocol::{Calibration, DeviceMsg, HostMsg};

use crate::motor;
use crate::stream;

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

        embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        )
    };

    static DEVICE_HANDLER: StaticCell<StopOnDisconnect> = StaticCell::new();
    builder.handler(DEVICE_HANDLER.init(StopOnDisconnect));

    let class = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        CdcAcmClass::new(&mut builder, state, 64)
    };
    let (sender, receiver, control) = class.split_with_control();

    let usb = builder.build();

    spawner.spawn(usb_task(usb).unwrap());
    spawner.spawn(tx_task(sender).unwrap());
    spawner.spawn(rx_task(receiver).unwrap());
    spawner.spawn(dtr_task(control).unwrap());
}

type UsbDriver = Driver<'static, USB>;

/// Stops the motor when the host goes away without clearing DTR (cable pulled, PC asleep).
struct StopOnDisconnect;

impl Handler for StopOnDisconnect {
    fn reset(&mut self) {
        motor::stop();
    }

    fn configured(&mut self, configured: bool) {
        if !configured {
            motor::stop();
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            motor::stop();
        }
    }
}

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

/// Stops the motor as soon as the host closes the port.
#[embassy_executor::task]
async fn dtr_task(control: ControlChanged<'static>) -> ! {
    loop {
        control.control_changed().await;
        if !control.dtr() {
            motor::stop();
        }
    }
}

#[embassy_executor::task]
async fn tx_task(mut sender: Sender<'static, UsbDriver>) {
    let cal = CALIBRATION.wait().await;
    loop {
        sender.wait_connection().await;
        while !sender.dtr() {
            embassy_time::Timer::after_millis(10).await;
        }
        stream::clear(); // drop stale blocks from before the connect

        // Every new host needs the calibration first
        if send(&mut sender, &DeviceMsg::Calibration(cal)).await.is_err() {
            continue;
        }

        let mut status_ticker = Ticker::every(Duration::from_millis(100));
        while sender.dtr() {
            let msg = match select(stream::next(), status_ticker.next()).await {
                Either::First(block) => DeviceMsg::CurrentSamples(block),
                Either::Second(()) => DeviceMsg::Status(motor::status()),
            };
            if send(&mut sender, &msg).await.is_err() {
                break;
            }
        }
    }
}

async fn send(sender: &mut Sender<'static, UsbDriver>, msg: &DeviceMsg) -> Result<(), EndpointError> {
    let mut buf = [0; 256];
    let frame = to_slice_cobs(msg, &mut buf).unwrap();
    for chunk in frame.chunks(64) {
        sender.write_packet(chunk).await?;
    }
    Ok(())
}

#[embassy_executor::task]
async fn rx_task(mut receiver: Receiver<'static, UsbDriver>) {
    let mut packet = [0u8; 64];

    loop {
        receiver.wait_connection().await;

        // A HostMsg encodes to a few bytes, longer frames are garbage
        let mut frame = [0u8; 16];
        let mut len = 0;
        let mut overflow = false;

        loop {
            let n = match receiver.read_packet(&mut packet).await {
                Ok(n) => n,
                Err(_) => break,
            };
            for &b in &packet[..n] {
                if len < frame.len() {
                    frame[len] = b;
                    len += 1;
                } else {
                    overflow = true;
                }
                // 0x00 terminates a COBS frame
                if b == 0 {
                    if !overflow {
                        handle(&mut frame[..len]);
                    }
                    len = 0;
                    overflow = false;
                }
            }
        }
    }
}

fn handle(frame: &mut [u8]) {
    let Ok(msg) = from_bytes_cobs::<HostMsg>(frame) else {
        warn!("invalid host message");
        return;
    };
    match msg {
        HostMsg::Start => motor::start(),
        HostMsg::Stop => motor::stop(),
        HostMsg::SetPeriod(us) => motor::set_period(us),
    }
    info!("host: {}", defmt::Debug2Format(&msg));
}
