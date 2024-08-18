#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Uncomment to permanetly set nboot0 (BOOT0 is floating :D x_x)
    // _*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_
    // // Wait, while the memory interface is busy.
    // while pac::FLASH.sr().read().bsy() {}
    //
    // // Unlock flash
    // if pac::FLASH.cr().read().lock() {
    //     defmt::info!("Flash is locked, unlocking");
    //     /* Magic bytes from embassy-stm32/src/flash/g.rs / RM */
    //     pac::FLASH.keyr().write_value(0x4567_0123);
    //     pac::FLASH.keyr().write_value(0xCDEF_89AB);
    // }
    // // Check: Should be unlocked.
    // assert_eq!(pac::FLASH.cr().read().lock(), false);
    //
    // // Unlock Option bytes
    // if pac::FLASH.cr().read().optlock() {
    //     defmt::info!("Option bytes locked, unlocking");
    //
    //     /* Source: RM / original HAL */
    //     pac::FLASH.optkeyr().write_value(0x0819_2A3B);
    //     pac::FLASH.optkeyr().write_value(0x4C5D_6E7F);
    // }
    // // Check: Should be unlocked
    // assert_eq!(pac::FLASH.cr().read().optlock(), false);
    //
    // /* Program boot0 */
    // pac::FLASH.optr().modify(|r| {
    //     r.set_n_boot0(true);
    //     r.set_n_swboot0(false);
    // });
    //
    // // Check: Should have changed
    // assert_eq!(pac::FLASH.optr().read().n_boot0(), true);
    // assert_eq!(pac::FLASH.optr().read().n_swboot0(), false);
    //
    // // Reload option bytes. This should in general cause RESET.
    // pac::FLASH.cr().modify(|w| w.set_optstrt(true));
    // while pac::FLASH.sr().read().bsy() {}
    //
    // pac::FLASH.cr().modify(|w| w.set_obl_launch(true));
    //
    // defmt::info!("Relocking");
    // // Lock option bytes and flash
    // pac::FLASH.cr().modify(|w| w.set_optlock(true));
    // pac::FLASH.cr().modify(|w| w.set_lock(true));
    // _*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_*_

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

    let mut adc = Adc::new(p.ADC2);
    adc.set_resolution(embassy_stm32::adc::Resolution::BITS12);
    adc.set_sample_time(SampleTime::CYCLES24_5);

    loop {
        let measured: f32 = adc.blocking_read(&mut p.PA6).into();
        let voltage: f32 = ((3.3 / 4095.0) * measured) / (10.0 / 57.0);
        info!("Battery voltage: {} V", voltage);
        Timer::after_millis(500).await;
    }
}
