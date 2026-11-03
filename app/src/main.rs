#![no_std]
#![no_main]

mod fmt;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;

mod current_sense;
mod stream;
mod usb_telem;

#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use core::sync::atomic::{AtomicU8, Ordering};

use embassy_executor::Spawner;
use embassy_stm32::{
    adc::{Adc, AdcChannel, AdcConfig, InjectedTrigger, Resolution, SampleTime},
    bind_interrupts,
    gpio::{Level, Output, OutputType, Speed},
    peripherals::ADC1,
    time::khz,
    timer::{
        complementary_pwm::{ComplementaryPwm, ComplementaryPwmPin, Mms2},
        simple_pwm::PwmPin,
        Channel,
    },
};
use embassy_time::Timer;
use fmt::info;

pub struct Tim1Trgo2;
impl InjectedTrigger<ADC1> for Tim1Trgo2 {
    fn signal(&self) -> u8 {
        8
    }
}

/// Current commutation step (1..=6), 0 while stopped.
pub static STEP: AtomicU8 = AtomicU8::new(0);

bind_interrupts!( struct Irqs {
    ADC1_2 => current_sense::CurrentSenseIrq;
    USB_LP => embassy_stm32::usb::InterruptHandler<embassy_stm32::peripherals::USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        // Not in use for now
        // config.rcc.hse = Some(Hse {
        //     freq: Hertz(24_000_000),
        //     mode: HseMode::Oscillator,
        // });
        config.rcc.pll = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4, // 16 MHz / 4 = 4 MHz VCO input
            mul: PllMul::MUL40,      // 4 MHz * 40  = 160 MHz VCO
            divp: None,
            divq: None,
            divr: Some(PllRDiv::DIV2), // 160 MHz / 2 = 80 MHz SYSCLK
        });
        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.mux.adc12sel = mux::Adcsel::SYS;
        config.rcc.hsi48 = Some(Default::default());
        config.rcc.mux.clk48sel = mux::Clk48sel::HSI48;
    }
    let p = embassy_stm32::init(config);

    //USB
    usb_telem::init(spawner, p.USB, Irqs, p.PA12, p.PA11);

    //ADC
    let mut adc_config = AdcConfig::default();
    adc_config.resolution = Some(Resolution::BITS12);
    let mut adc = Adc::new(p.ADC1, adc_config);
    let mut vrefint = adc.enable_vrefint();
    let vrefint_raw = adc.blocking_read(&mut vrefint, SampleTime::CYCLES640_5);

    // Must stay alive: dropping it stops the ADC
    let _inj = adc.setup_injected_conversions(
        [
            (p.PB1.degrade_adc(), SampleTime::CYCLES12_5),
            (p.PB0.degrade_adc(), SampleTime::CYCLES12_5),
        ],
        Tim1Trgo2,
        embassy_stm32::adc::Exten::RISING_EDGE,
        true,
    );

    // MOTOR
    let mut en = Output::new(p.PA15, Level::Low, Speed::Low);

    let ch1 = PwmPin::new(p.PA8, OutputType::PushPull);
    let ch1n = ComplementaryPwmPin::new(p.PB13, OutputType::PushPull);
    let ch2 = PwmPin::new(p.PA9, OutputType::PushPull);
    let ch2n = ComplementaryPwmPin::new(p.PB14, OutputType::PushPull);
    let ch3 = PwmPin::new(p.PA10, OutputType::PushPull);
    let ch3n = ComplementaryPwmPin::new(p.PB15, OutputType::PushPull);

    let mut pwm = ComplementaryPwm::new(
        p.TIM1,
        Some(ch1),
        Some(ch1n),
        Some(ch2),
        Some(ch2n),
        Some(ch3),
        Some(ch3n),
        None,
        None,
        khz(20),
        embassy_stm32::timer::low_level::CountingMode::CenterAlignedUpInterrupts,
    );

    let max = pwm.get_max_duty();
    pwm.set_dead_time((max / 1024) as u16);
    pwm.set_duty(Channel::Ch4, max - 1);
    pwm.set_mms2(Mms2::COMPARE_OC4);

    let duty = max / 10;

    pwm.disable(Channel::Ch1);
    pwm.disable(Channel::Ch2);
    pwm.disable(Channel::Ch3);

    Timer::after_millis(100).await;

    current_sense::enable(Irqs);

    en.set_high();
    Timer::after_millis(10).await;

    // Power stage still off, so this is the zero-current offset
    let cal = current_sense::calibrate(vrefint_raw, 256).await;
    usb_telem::set_calibration(cal);
    let (bias_a_mv, bias_c_mv) = cal.bias_mv();
    info!("bias_a: {} mV | bias_c: {} mV", bias_a_mv, bias_c_mv);

    let mut step: u8 = 1;
    let mut period_us: u64 = 6000;
    let mut log_div: u32 = 0;

    loop {
        //(Quelle = +V/PWM, Senke = GND/statisch, floatend)   A=Ch1  B=Ch2  C=Ch3
        let (src, sink, flt) = match step {
            1 => (Channel::Ch1, Channel::Ch3, Channel::Ch2), // A→C
            2 => (Channel::Ch1, Channel::Ch2, Channel::Ch3), // A→B
            3 => (Channel::Ch3, Channel::Ch2, Channel::Ch1), // C→B
            4 => (Channel::Ch3, Channel::Ch1, Channel::Ch2), // C→A
            5 => (Channel::Ch2, Channel::Ch1, Channel::Ch3), // B→A
            _ => (Channel::Ch2, Channel::Ch3, Channel::Ch1), // B→C
        };

        pwm.disable(flt); // beide FETs aus → Phase floatet
        pwm.set_duty(sink, 0); // Duty 0 → Low-Side dauerhaft an
        pwm.enable(sink);
        pwm.set_duty(src, duty); // High-Side choppt, Low-Side = Komplement
        pwm.enable(src);
        STEP.store(step, Ordering::Relaxed);

        log_div += 1;
        if log_div >= 50 {
            log_div = 0;
            let (raw_a, raw_c) = current_sense::latest();
            let (i_a, i_c) = cal.to_amps(raw_a, raw_c);
            info!("step {} | i_a: {} A | i_c: {} A", step, i_a, i_c);
        }

        Timer::after_micros(period_us).await;

        step = if step >= 6 { 1 } else { step + 1 };
        if period_us > 1000 {
            period_us -= 5;
        }
    }
}
