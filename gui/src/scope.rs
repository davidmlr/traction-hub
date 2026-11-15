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

// Time budget of the ISR: one PWM period
const ISR_BUDGET_US: f32 = 50.0;

/// A sample with the ISR timing of the block it came in.
#[derive(Clone, Copy)]
struct Point {
    sample: Sample,
    isr_run_us: f32,
    isr_end_us: f32,
}

/// Oscilloscope-like view of the phase currents, triggered on the start of an electrical period.
pub struct Scope {
    history: VecDeque<Point>,
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
        let isr_run_us = block.isr_run_max_ns() / 1000.0;
        let isr_end_us = block.isr_end_max_ns() / 1000.0;
        self.history.extend(block.samples.iter().map(|&sample| Point {
            sample,
            isr_run_us,
            isr_end_us,
        }));
        let excess = self.history.len().saturating_sub(HISTORY_LEN);
        self.history.drain(..excess);
    }

    /// Index range of the latest `periods` complete electrical periods.
    fn triggered_window(&self) -> Option<Range<usize>> {
        let h = &self.history;
        let triggers: Vec<usize> = (1..h.len())
            .filter(|&i| h[i].sample.step == TRIGGER_STEP && h[i - 1].sample.step != TRIGGER_STEP)
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
        let mut isr_run = Vec::with_capacity(window.len());
        let mut isr_end = Vec::with_capacity(window.len());
        let (mut run_max, mut end_max) = (0.0f32, 0.0f32);
        for (n, p) in self.history.range(window).enumerate() {
            let t = n as f64 * SAMPLE_PERIOD_MS;
            let s = p.sample;
            let (a, c) = cal.to_amps(s.i_a, s.i_c);
            i_a.push([t, a as f64]);
            i_b.push([t, -(a + c) as f64]);
            i_c.push([t, c as f64]);
            step.push([t, s.step as f64]);
            isr_run.push([t, p.isr_run_us as f64]);
            isr_end.push([t, p.isr_end_us as f64]);
            run_max = run_max.max(p.isr_run_us);
            end_max = end_max.max(p.isr_end_us);
        }

        ui.label(format!(
            "ISR max in view: run {run_max:.2} µs | end {end_max:.2} µs ({:.0} % of {ISR_BUDGET_US:.0} µs)",
            end_max / ISR_BUDGET_US * 100.0
        ));

        let link = egui::Id::unique("scope_x");
        let running = self.running;
        let height = ui.available_height();

        Plot::new("currents")
            .height(height * 0.6)
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

        Plot::new("isr")
            .height(height * 0.2)
            .legend(Legend::default())
            .y_axis_label("ISR µs")
            .include_y(0.0)
            .link_axis(link, [true, false])
            .link_cursor(link, [true, false])
            .show(ui, |p| {
                if running {
                    p.set_auto_bounds(true);
                }
                p.line(Line::new("run", PlotPoints::from(isr_run)));
                p.line(Line::new("end", PlotPoints::from(isr_end)));
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
