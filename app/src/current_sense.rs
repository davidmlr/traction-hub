use core::sync::atomic::{AtomicU32, Ordering};

use crate::STEP;
use embassy_stm32::{
    interrupt::{
        self,
        typelevel::{self, Binding, Handler, Interrupt},
    },
    pac,
};
use embassy_time::Timer;
use protocol::Calibration;
use protocol::Sample;

use crate::stream::record;

pub struct CurrentSenseIrq;

// Phase A in upper 16 bits, phase C in lower 16 bits, so both come from the same sample
static LATEST: AtomicU32 = AtomicU32::new(0);

impl Handler<interrupt::typelevel::ADC1_2> for CurrentSenseIrq {
    unsafe fn on_interrupt() {
        let adc = pac::ADC1;

        let i_a = adc.jdr(0).read().jdata();
        let i_c = adc.jdr(1).read().jdata();

        record(Sample::new(i_a, i_c, STEP.load(Ordering::Relaxed)));

        LATEST.store(((i_a as u32) << 16) | i_c as u32, Ordering::Relaxed);

        // write, not modify: ISR flags are write-1-to-clear
        adc.isr().write(|w| w.set_jeos(true));
    }
}

pub fn enable(_irq: impl Binding<typelevel::ADC1_2, CurrentSenseIrq>) {
    typelevel::ADC1_2::unpend();
    typelevel::ADC1_2::set_priority(interrupt::Priority::P0);
    unsafe {
        typelevel::ADC1_2::enable();
    }
}

/// Latest raw ADC values (phase A, phase C).
pub fn latest() -> (u16, u16) {
    let v = LATEST.load(Ordering::Relaxed);
    ((v >> 16) as u16, v as u16)
}

/// Averages the zero-current offset. Interrupt must be enabled, power stage must be off.
pub async fn calibrate(vrefint_raw: u16, samples: u32) -> Calibration {
    let (mut sum_a, mut sum_c) = (0u32, 0u32);
    for _ in 0..samples {
        let (a, c) = latest();
        sum_a += a as u32;
        sum_c += c as u32;
        // Longer than one PWM period (50 µs), so every read is a new sample
        Timer::after_micros(100).await;
    }

    Calibration {
        vrefint_raw,
        bias_a: (sum_a / samples) as u16,
        bias_c: (sum_c / samples) as u16,
    }
}
