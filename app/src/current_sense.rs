use core::sync::atomic::{AtomicU32, Ordering};

use embassy_stm32::{
    interrupt::{
        self,
        typelevel::{self, Binding, Handler, Interrupt},
    },
    pac,
};
use embassy_time::Timer;

// Typical VREFINT, datasheet 6.3.24
const VREFINT_MV: f32 = 1210.0;
const AMP_GAIN: f32 = 10.0;
const SHUNT_OHM: f32 = 0.001;

pub struct CurrentSenseIrq;

// Phase A in upper 16 bits, phase C in lower 16 bits, so both come from the same sample
static LATEST: AtomicU32 = AtomicU32::new(0);

impl Handler<interrupt::typelevel::ADC1_2> for CurrentSenseIrq {
    unsafe fn on_interrupt() {
        let adc = pac::ADC1;

        let i_a = adc.jdr(0).read().jdata();
        let i_c = adc.jdr(1).read().jdata();

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

pub struct Calibration {
    mv_per_lsb: f32,
    bias_a: u16,
    bias_c: u16,
}

impl Calibration {
    /// Averages the zero-current offset. Interrupt must be enabled, power stage must be off.
    pub async fn measure(vrefint_raw: u16, samples: u32) -> Self {
        let (mut sum_a, mut sum_c) = (0u32, 0u32);
        for _ in 0..samples {
            let (a, c) = latest();
            sum_a += a as u32;
            sum_c += c as u32;
            // Longer than one PWM period (50 µs), so every read is a new sample
            Timer::after_micros(100).await;
        }

        Self {
            mv_per_lsb: VREFINT_MV / vrefint_raw as f32,
            bias_a: (sum_a / samples) as u16,
            bias_c: (sum_c / samples) as u16,
        }
    }

    pub fn bias_mv(&self) -> (f32, f32) {
        (
            self.bias_a as f32 * self.mv_per_lsb,
            self.bias_c as f32 * self.mv_per_lsb,
        )
    }

    /// Converts raw ADC values to phase currents in ampere.
    pub fn to_amps(&self, raw_a: u16, raw_c: u16) -> (f32, f32) {
        (
            self.raw_to_amps(raw_a, self.bias_a),
            self.raw_to_amps(raw_c, self.bias_c),
        )
    }

    fn raw_to_amps(&self, raw: u16, bias: u16) -> f32 {
        let delta = raw as i32 - bias as i32;
        let shunt_mv = delta as f32 * self.mv_per_lsb / AMP_GAIN;
        shunt_mv / 1000.0 / SHUNT_OHM
    }
}
