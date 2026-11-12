//! Open-loop 6-step state machine, clocked by the ADC interrupt (one tick per PWM period).
//! Tasks only write the command atomics, the ISR applies them.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};

use crate::commutation;

pub static CMD_RUN: AtomicBool = AtomicBool::new(false);
/// Target time per commutation step in µs
pub static CMD_PERIOD_US: AtomicU32 = AtomicU32::new(1000);

const TICK_US: u32 = 50;
const START_PERIOD_US: u32 = 6000;
const RAMP_US_PER_STEP: u32 = 5;

static MOTOR: Mutex<CriticalSectionRawMutex, RefCell<Motor>> =
    Mutex::new(RefCell::new(Motor::new()));

struct Motor {
    running: bool,
    step: u8,
    elapsed_us: u32,
    period_us: u32,
}

impl Motor {
    const fn new() -> Self {
        Self {
            running: false,
            step: 0,
            elapsed_us: 0,
            period_us: START_PERIOD_US,
        }
    }

    fn tick(&mut self) -> u8 {
        let run = CMD_RUN.load(Relaxed);
        if run && !self.running {
            self.running = true;
            self.step = 1;
            self.elapsed_us = 0;
            self.period_us = START_PERIOD_US;
            commutation::commutate(self.step);
        } else if !run && self.running {
            self.running = false;
            self.step = 0;
            commutation::all_off();
        }
        if !self.running {
            return 0;
        }

        self.elapsed_us += TICK_US;
        if self.elapsed_us >= self.period_us {
            // Keep the remainder so the average period stays exact
            self.elapsed_us -= self.period_us;
            self.step = if self.step >= 6 { 1 } else { self.step + 1 };
            commutation::commutate(self.step);
            self.ramp(CMD_PERIOD_US.load(Relaxed));
        }
        self.step
    }

    fn ramp(&mut self, target: u32) {
        if self.period_us > target {
            self.period_us -= RAMP_US_PER_STEP.min(self.period_us - target);
        } else {
            self.period_us += RAMP_US_PER_STEP.min(target - self.period_us);
        }
    }
}

/// Called once per PWM period from the ADC ISR. Returns the current step (0 = stopped).
pub fn on_pwm_tick() -> u8 {
    MOTOR.lock(|m| m.borrow_mut().tick())
}
