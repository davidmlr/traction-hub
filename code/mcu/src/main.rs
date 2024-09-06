#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(24_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV6,
            mul: PllMul::MUL85,
            divp: None,
            divq: None,
            // Main system clock at 170 MHz
            divr: Some(PllRDiv::DIV2),
        });
        config.rcc.mux.adc12sel = mux::Adcsel::SYS;
        config.rcc.sys = Sysclk::PLL1_R;
    }
    let mut p = embassy_stm32::init(config);
    info!("Log");

    let mut adc2 = Adc::new(p.ADC2);
    adc2.set_resolution(embassy_stm32::adc::Resolution::BITS12);
    adc2.set_sample_time(SampleTime::CYCLES24_5);

    let mut adc1 = Adc::new(p.ADC1);
    adc1.set_resolution(embassy_stm32::adc::Resolution::BITS12);
    adc1.set_sample_time(SampleTime::CYCLES24_5);

    let octw = Input::new(p.PC14, Pull::Down);
    let fault = Input::new(p.PC13, Pull::Down);

    // let mut dc_cal = Output::new(p.PC15, Level::Low, Speed::Low);

    let mut h1 = Output::new(p.PA10, Level::Low, Speed::Medium);
    let mut l1 = Output::new(p.PB15, Level::Low, Speed::Medium);

    let mut h2 = Output::new(p.PA9, Level::Low, Speed::Medium);
    let mut l2 = Output::new(p.PB14, Level::Low, Speed::Medium);

    let mut h3 = Output::new(p.PA8, Level::Low, Speed::Medium);
    let mut l3 = Output::new(p.PB13, Level::Low, Speed::Medium);

    let mut en = Output::new(p.PA15, Level::Low, Speed::Medium);

    Timer::after_millis(300).await;
    en.set_high();
    for _i in 0..100 {
        if fault.is_high() || octw.is_high() {
            en.set_low();
            error!("DRV8302 ERROR");
        }
        for step in 0..6 {
            set_gates(step, &mut h1, &mut h2, &mut h3, &mut l1, &mut l2, &mut l3);
            Timer::after_millis(10).await;
            set_gates(6, &mut h1, &mut h2, &mut h3, &mut l1, &mut l2, &mut l3);
            Timer::after_millis(200).await;
        }
    }
    loop {
        let measured: f32 = adc2.blocking_read(&mut p.PA6).into();
        let measured_sens1: f32 = adc1.blocking_read(&mut p.PA2).into();
        let measured_sens2: f32 = adc2.blocking_read(&mut p.PA1).into();
        let measured_sens3: f32 = adc2.blocking_read(&mut p.PA0).into();
        let measured_s01: f32 = adc1.blocking_read(&mut p.PB0).into();
        let measured_s02: f32 = adc1.blocking_read(&mut p.PB1).into();
        let voltage_vcc: f32 = 3.3 / 4095.0 * measured * 5.7;
        let voltage_sens1: f32 = 3.3 / 4095.0 * measured_sens1 * 41.2 / 2.2;
        let voltage_sens2: f32 = 3.3 / 4095.0 * measured_sens2 * 41.2 / 2.2;
        let voltage_sens3: f32 = 3.3 / 4095.0 * measured_sens3 * 41.2 / 2.2;
        let current_s01: f32 = 3.3 / 4095.0 * measured_s01;
        let current_s02: f32 = 3.3 / 4095.0 * measured_s02;
        info!("Battery voltage: {} V", voltage_vcc);
        info!(
            "Sens3 voltage: {} V | Sens2 voltage: {} V | Sens1 voltage: {} V",
            voltage_sens3, voltage_sens2, voltage_sens1
        );
        info!(
            "Current SO1: {} I | Current SO2: {} I",
            current_s01, current_s02
        );
        if fault.is_high() || octw.is_high() {
            en.set_low();
            error!("DRV8302 ERROR");
        }
        Timer::after_millis(500).await;
    }
}

fn set_gates(
    step: u8,
    h1: &mut Output,
    h2: &mut Output,
    h3: &mut Output,
    l1: &mut Output,
    l2: &mut Output,
    l3: &mut Output,
) {
    info!("gates function {}", step);
    match step {
        0 => {
            h1.set_high();
            l1.set_low();
            h2.set_low();
            l2.set_high();
            h3.set_low();
            l3.set_low();
        }
        1 => {
            h1.set_high();
            l1.set_low();
            h2.set_low();
            l2.set_low();
            h3.set_low();
            l3.set_high();
        }
        2 => {
            h1.set_low();
            l1.set_low();
            h2.set_high();
            l2.set_low();
            h3.set_low();
            l3.set_high();
        }
        3 => {
            h1.set_low();
            l1.set_high();
            h2.set_high();
            l2.set_low();
            h3.set_low();
            l3.set_low();
        }
        4 => {
            h1.set_low();
            l1.set_high();
            h2.set_low();
            l2.set_low();
            h3.set_high();
            l3.set_low();
        }
        5 => {
            h1.set_low();
            l1.set_low();
            h2.set_low();
            l2.set_high();
            h3.set_high();
            l3.set_low();
        }
        // All low
        6 => {
            l1.set_low();
            l2.set_low();
            l3.set_low();
            h1.set_low();
            h2.set_low();
            h3.set_low();
        }
        _ => error!("Incorrect step"),
    };
}
