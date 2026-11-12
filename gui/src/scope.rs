use std::collections::VecDeque;
use std::ops::Range;

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use protocol::{Calibration, Sample, SampleBlock};

// One sample per PWM period (20 kHz)
const SAMPLE_PERIOD_MS: f64 = 0.05;
// 1 s of history
const HISTORY_LEN: usize = 20_000;
// Shown when no trigger is found (motor stopped)
const UNTRIGGERED_LEN: usize = 1_000;
const TRIGGER_STEP: u8 = 1;

/// Oscilloscope-like view of the phase currents, triggered on the start of an electrical period.
pub struct Scope {
    history: VecDeque<Sample>,
    periods: usize,
    running: bool,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(HISTORY_LEN),
            periods: 3,
            running: true,
        }
    }

    pub fn push(&mut self, block: &SampleBlock) {
        if !self.running {
            return;
        }
        self.history.extend(block.samples.iter().copied());
        let excess = self.history.len().saturating_sub(HISTORY_LEN);
        self.history.drain(..excess);
    }

    /// Index range of the latest `periods` complete electrical periods.
    fn triggered_window(&self) -> Option<Range<usize>> {
        let h = &self.history;
        let triggers: Vec<usize> = (1..h.len())
            .filter(|&i| h[i].step == TRIGGER_STEP && h[i - 1].step != TRIGGER_STEP)
            .collect();
        let end = *triggers.last()?;
        let start = *triggers.iter().rev().nth(self.periods)?;
        Some(start..end)
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, cal: &Calibration) {
        ui.horizontal(|ui| {
            let label = if self.running { "Stop" } else { "Run" };
            if ui.button(label).clicked() {
                self.running = !self.running;
            }
            ui.add(egui::Slider::new(&mut self.periods, 1..=10).text("periods"));
        });

        let window = match self.triggered_window() {
            Some(w) => w,
            None => {
                ui.label("No trigger, showing untriggered");
                self.history.len().saturating_sub(UNTRIGGERED_LEN)..self.history.len()
            }
        };

        let mut i_a = Vec::with_capacity(window.len());
        let mut i_b = Vec::with_capacity(window.len());
        let mut i_c = Vec::with_capacity(window.len());
        let mut step = Vec::with_capacity(window.len());
        for (n, s) in self.history.range(window).enumerate() {
            let t = n as f64 * SAMPLE_PERIOD_MS;
            let (a, c) = cal.to_amps(s.i_a, s.i_c);
            i_a.push([t, a as f64]);
            i_b.push([t, -(a + c) as f64]);
            i_c.push([t, c as f64]);
            step.push([t, s.step as f64]);
        }

        let link = egui::Id::unique("scope_x");
        let running = self.running;
        let height = ui.available_height();

        Plot::new("currents")
            .height(height * 0.75)
            .legend(Legend::default())
            .y_axis_label("A")
            .link_axis(link, [true, false])
            .link_cursor(link, [true, false])
            .show(ui, |p| {
                if running {
                    p.set_auto_bounds(true);
                }
                p.line(Line::new("I_a", PlotPoints::from(i_a)));
                p.line(Line::new("I_b", PlotPoints::from(i_b)));
                p.line(Line::new("I_c", PlotPoints::from(i_c)));
            });

        Plot::new("step")
            .height(ui.available_height())
            .x_axis_label("ms")
            .y_axis_label("step")
            .include_y(0.0)
            .include_y(6.0)
            .link_axis(link, [true, false])
            .link_cursor(link, [true, false])
            .show(ui, |p| {
                if running {
                    p.set_auto_bounds(true);
                }
                p.line(Line::new("step", PlotPoints::from(step)));
            });
    }
}
