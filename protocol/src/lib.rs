#![no_std]

use serde::{Deserialize, Serialize};

// Typical VREFINT, datasheet 6.3.24
const VREFINT_MV: f32 = 1210.0;
const AMP_GAIN: f32 = 10.0;
const SHUNT_OHM: f32 = 0.001;

/// Messages from the device to the host.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DeviceMsg {
    Calibration(Calibration),
    Telemetry(Telemetry),
}

/// Raw ADC values, convert with [`Calibration::to_amps`].
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Telemetry {
    pub i_a: u16,
    pub i_c: u16,
    pub step: u8,
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
