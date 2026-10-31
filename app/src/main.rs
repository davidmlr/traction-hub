#![no_std]
#![no_main]

mod fmt;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;

#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_stm32::{
    adc::{Adc, AdcConfig, Resolution, SampleTime},
    gpio::{Level, Output, OutputType, Speed},
    time::khz,
    timer::{
        complementary_pwm::{ComplementaryPwm, ComplementaryPwmPin},
        simple_pwm::PwmPin,
        Channel,
    },
};
use embassy_time::Timer;
use fmt::info;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
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

    //ADC
    let mut adc_config = AdcConfig::default();
    adc_config.resolution = Some(Resolution::BITS12);
    let mut adc = Adc::new(p.ADC1, adc_config);
    let mut sens1 = p.PB1;
    let mut sens2 = p.PB0;
    let mut vrefint = adc.enable_vrefint();
    let vrefint_sample = adc.blocking_read(&mut vrefint, SampleTime::CYCLES640_5);

    let convert_to_millivolts = |sample: u16| {
        // From http://www.st.com/resource/en/datasheet/DM00071990.pdf
        // 6.3.24 Reference voltage
        const VREFINT_MV: u32 = 1210; // mV

        (u32::from(sample) * VREFINT_MV / u32::from(vrefint_sample)) as u16
    };

    let convert_to_current = |sample| {
        let sample_mv: u16 = convert_to_millivolts(sample);
        let gain = 10 as f32;
        let bias = 1650 as u16; // mV
        let sens_res = 0.001 as f32; // Ohm
        let shunt_voltage: f32;

        if sample_mv >= bias {
            shunt_voltage = (sample_mv - bias) as f32 / gain;
        } else {
            shunt_voltage = (bias - sample_mv) as f32 / -gain;
        }
        return shunt_voltage / sens_res;
    };

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
        Default::default(),
    );

    let max = pwm.get_max_duty();
    pwm.set_dead_time((max / 1024) as u16);

    let duty = max / 10;

    pwm.disable(Channel::Ch1);
    pwm.disable(Channel::Ch2);
    pwm.disable(Channel::Ch3);

    Timer::after_millis(100).await;

    let mut step: u8 = 1;
    let mut period_us: u64 = 6000;

    en.set_high();

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

        let vsens1 = adc.blocking_read(&mut sens1, SampleTime::CYCLES640_5);
        let vsens2 = adc.blocking_read(&mut sens2, SampleTime::CYCLES640_5);

        info!(
            "i_a: {}, i_c: {}",
            convert_to_current(vsens1),
            convert_to_current(vsens2)
        );

        Timer::after_micros(period_us).await;

        step = if step >= 6 { 1 } else { step + 1 };
        if period_us > 1000 {
            period_us -= 5;
        }
    }
}
