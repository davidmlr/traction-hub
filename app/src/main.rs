#![no_std]
#![no_main]

mod fmt;
mod telemetry;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Level, Output, Speed},
    peripherals,
    time::Hertz,
    usb::{self, Driver},
};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder,
};
use fmt::info;
use static_cell::StaticCell;
use telemetry::Telemetry;

bind_interrupts!(struct Irqs {
    USB_LP => usb::InterruptHandler<peripherals::USB>;
});

type UsbDriver = Driver<'static, peripherals::USB>;

static CDC_STATE: StaticCell<State> = StaticCell::new();
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

#[embassy_executor::task]
async fn usb_task(mut device: embassy_usb::UsbDevice<'static, UsbDriver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn telemetry_task(mut class: CdcAcmClass<'static, UsbDriver>) {
    loop {
        class.wait_connection().await;
        info!("USB host connected");

        loop {
            let frame = Telemetry {
                timestamp_ms: Instant::now().as_millis() as u32,
                ..Default::default()
            };

            let mut buf = [0u8; 32];
            match postcard::to_slice_cobs(&frame, &mut buf) {
                Ok(encoded) => {
                    if class.write_packet(encoded).await.is_err() {
                        break;
                    }
                }
                Err(_) => {}
            }

            Timer::after(Duration::from_millis(10)).await; // 100 Hz
        }

        info!("USB host disconnected");
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(24_000_000),
            mode: HseMode::Oscillator, // crystal; use HseMode::Bypass for clock gen IC
        });
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV6,    // 24 MHz / 6 = 4 MHz VCO input
            mul: PllMul::MUL85,         // 4 MHz * 85  = 340 MHz VCO
            divp: None,
            divq: None,
            divr: Some(PllRDiv::DIV2),  // 340 MHz / 2 = 170 MHz SYSCLK
        });
        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.mux.adc12sel = mux::Adcsel::SYS;
        // HSI48 as USB 48 MHz clock source (no PLL changes needed)
        config.rcc.hsi48 = Some(Default::default());
        config.rcc.mux.clk48sel = mux::Clk48sel::HSI48;
    }
    let p = embassy_stm32::init(config);

    let mut led = Output::new(p.PB8, Level::High, Speed::Low);

    // USB FS: PA12 = D+, PA11 = D-
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("TractionHub");
    usb_config.product = Some("Motor Controller");
    usb_config.serial_number = Some("001");
    usb_config.max_power = 100; // mA

    let mut builder = Builder::new(
        driver,
        usb_config,
        CONFIG_DESC.init([0u8; 256]),
        BOS_DESC.init([0u8; 256]),
        MSOS_DESC.init([0u8; 256]),
        CONTROL_BUF.init([0u8; 64]),
    );

    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);
    let usb = builder.build();

    spawner.spawn(usb_task(usb).unwrap());
    spawner.spawn(telemetry_task(class).unwrap());

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}
