//! Panic and HardFault handling: power stage off first, then report.

use cortex_m::peripheral::SCB;
use cortex_m_rt::{exception, ExceptionFrame};
use embassy_stm32::pac;

/// Turns off the power stage without relying on any driver state.
fn power_off() {
    pac::TIM1.bdtr().modify(|w| w.set_moe(false));
    // Gate driver enable (PA15) low
    pac::GPIOA.bsrr().write(|w| w.set_br(15, true));
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    power_off();
    #[cfg(feature = "defmt")]
    defmt::error!("{}", defmt::Display2Format(info));
    #[cfg(not(feature = "defmt"))]
    let _ = info;
    cortex_m::asm::udf()
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    power_off();

    #[cfg(feature = "defmt")]
    {
        let scb = unsafe { &*SCB::PTR };
        defmt::error!(
            "HardFault: pc={=u32:#010x} lr={=u32:#010x} cfsr={=u32:#010x} hfsr={=u32:#010x} mmfar={=u32:#010x} bfar={=u32:#010x}",
            ef.pc(),
            ef.lr(),
            scb.cfsr.read(),
            scb.hfsr.read(),
            scb.mmfar.read(),
            scb.bfar.read(),
        );
    }
    #[cfg(not(feature = "defmt"))]
    let _ = ef;

    loop {
        cortex_m::asm::nop();
    }
}
