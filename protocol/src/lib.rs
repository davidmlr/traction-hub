#![no_std]

use serde::{Deserialize, Serialize};

// Typical VREFINT, datasheet 6.3.24
const VREFINT_MV: f32 = 1210.0;
const AMP_GAIN: f32 = 10.0;
const SHUNT_OHM: f32 = 0.001;

pub const BLOCK_LEN: usize = 32;
/// Duration of one TIM1 tick (80 MHz)
pub const TIM1_TICK_NS: f32 = 12.5;

/// Messages from the device to the host.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DeviceMsg {
    Calibration(Calibration),
    CurrentSamples(SampleBlock),
    Status(Status),
}

/// Messages from the host to the device.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum HostMsg {
    Start,
    Stop,
    /// Target time per commutation step in µs
    SetPeriod(u32),
}

/// Slowly changing motor state, sent periodically.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Status {
    pub running: bool,
    /// Current time per commutation step in µs (follows the target via a ramp)
    pub period_us: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Sample {
    pub i_a: u16,
    pub i_c: u16,
    pub step: u8,
}

impl Sample {
    pub const fn new(i_a: u16, i_c: u16, step: u8) -> Self {
        Sample { i_a, i_c, step }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SampleBlock {
    pub seq: u32,
    /// Longest ISR run time (entry to exit) within this block, TIM1 ticks
    pub isr_run_max_ticks: u16,
    /// Latest ISR end (ADC trigger to exit) within this block, TIM1 ticks
    pub isr_end_max_ticks: u16,
    pub samples: [Sample; BLOCK_LEN],
}

impl SampleBlock {
    pub const fn new() -> Self {
        Self {
            seq: 0,
            isr_run_max_ticks: 0,
            isr_end_max_ticks: 0,
            samples: [Sample::new(0, 0, 0); BLOCK_LEN],
        }
    }

    pub fn isr_run_max_ns(&self) -> f32 {
        self.isr_run_max_ticks as f32 * TIM1_TICK_NS
    }

    pub fn isr_end_max_ns(&self) -> f32 {
        self.isr_end_max_ticks as f32 * TIM1_TICK_NS
    }
}

/// Raw ADC values measured at startup with the power stage off.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Calibration {
    pub vrefint_raw: u16,
    pub bias_a: u16,
    pub bias_c: u16,
}

impl Calibration {
    pub fn mv_per_lsb(&self) -> f32 {
        VREFINT_MV / self.vrefint_raw as f32
    }

    pub fn bias_mv(&self) -> (f32, f32) {
        (
            self.bias_a as f32 * self.mv_per_lsb(),
            self.bias_c as f32 * self.mv_per_lsb(),
        )
    }

    /// Converts raw ADC values (phase A, phase C) to phase currents in ampere.
    pub fn to_amps(&self, raw_a: u16, raw_c: u16) -> (f32, f32) {
        (
            self.raw_to_amps(raw_a, self.bias_a),
            self.raw_to_amps(raw_c, self.bias_c),
        )
    }

    fn raw_to_amps(&self, raw: u16, bias: u16) -> f32 {
        let delta = raw as i32 - bias as i32;
        let shunt_mv = delta as f32 * self.mv_per_lsb() / AMP_GAIN;
        shunt_mv / 1000.0 / SHUNT_OHM
    }
}
