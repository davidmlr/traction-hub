//! Glitch-free 6-step commutation on TIM1 using the COM event.
//!
//! All three CCRs share the same duty. A phase's role is set only via OCxM and
//! CCxE/CCxNE, which are preloaded (CCPC=1) and applied together by COMG.

use embassy_stm32::pac::{self, timer::vals::Ocm};

/// Enables preloading of OCxM/CCxE/CCxNE until the next COM event.
pub fn init() {
    pac::TIM1.cr2().modify(|w| w.set_ccpc(true));
}

/// Applies a commutation step 1..=6. Any other value turns all phases off.
pub fn commutate(step: u8) {
    // (source, sink) channel index, A=0 B=1 C=2
    let (src, sink) = match step {
        1 => (0, 2), // A→C
        2 => (0, 1), // A→B
        3 => (2, 1), // C→B
        4 => (2, 0), // C→A
        5 => (1, 0), // B→A
        6 => (1, 2), // B→C
        _ => return all_off(),
    };

    let tim = pac::TIM1;
    for ch in 0..3 {
        // Source chops, sink has REF forced low → low side permanently on
        let mode = if ch == src { Ocm::PWM_MODE1 } else { Ocm::FORCE_INACTIVE };
        tim.ccmr_output(ch / 2).modify(|w| w.set_ocm(ch % 2, mode));
    }
    tim.ccer().modify(|w| {
        for ch in 0..3 {
            let on = ch == src || ch == sink;
            w.set_cce(ch, on);
            w.set_ccne(ch, on);
        }
    });
    tim.egr().write(|w| w.set_comg(true));
}

/// All phases floating, motor coasts.
pub fn all_off() {
    let tim = pac::TIM1;
    tim.ccer().modify(|w| {
        for ch in 0..3 {
            w.set_cce(ch, false);
            w.set_ccne(ch, false);
        }
    });
    tim.egr().write(|w| w.set_comg(true));
}
