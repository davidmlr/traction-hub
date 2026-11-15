use eframe::egui;
use protocol::{BLOCK_LEN, Calibration, HostMsg, Status, PERIOD_US_MAX, PERIOD_US_MIN};

use crate::link::{Link, LinkEvent};
use crate::scope::Scope;

pub struct Session {
    link: Link,
    connected: bool,
    closed: Option<String>,
    calibration: Option<Calibration>,
    stats: Option<(f32, u64)>,
    status: Option<Status>,
    /// Target period set with the slider
    period_us: u32,
    scope: Scope,
}

impl Session {
    pub fn new(link: Link) -> Self {
        Self {
            link,
            connected: false,
            closed: None,
            calibration: None,
            stats: None,
            status: None,
            period_us: PERIOD_US_MIN,
            scope: Scope::new(),
        }
    }

    /// Applies all pending events from the link thread.
    pub fn poll(&mut self) {
        for ev in self.link.events.try_iter() {
            match ev {
                LinkEvent::Connected => self.connected = true,
                LinkEvent::Disconnected(reason) => self.closed = Some(reason),
                LinkEvent::Calibration(c) => self.calibration = Some(c),
                LinkEvent::Samples(block) => self.scope.push(&block),
                LinkEvent::Stats { blocks_per_s, lost } => self.stats = Some((blocks_per_s, lost)),
                LinkEvent::Status(st) => self.status = Some(st),
            }
        }
    }

    /// Reason the link thread ended, if it did.
    pub fn closed(&self) -> Option<&str> {
        self.closed.as_deref()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.connected {
            ui.label("Connecting...");
            return;
        }

        if let Some((rate, lost)) = self.stats {
            ui.label(format!(
                "{rate:.0} blocks/s | {:.0} samples/s | lost blocks: {lost}",
                rate * BLOCK_LEN as f32
            ));
        }

        self.controls(ui);
        ui.separator();

        match &self.calibration {
            Some(c) => {
                let (bias_a, bias_c) = c.bias_mv();
                ui.label(format!("Bias: A {bias_a:.1} mV, C {bias_c:.1} mV"));
                ui.separator();
                self.scope.ui(ui, c);
            }
            None => {
                ui.label("No calibration received yet (reset the board)");
            }
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let running = self.status.is_some_and(|st| st.running);

        ui.horizontal(|ui| {
            if ui.add_enabled(!running, egui::Button::new("Start")).clicked() {
                // Device may still have a period from an earlier session
                self.link.send(HostMsg::SetPeriod(self.period_us));
                self.link.send(HostMsg::Start);
            }
            if ui.add_enabled(running, egui::Button::new("Stop")).clicked() {
                self.link.send(HostMsg::Stop);
            }

            let slider = egui::Slider::new(&mut self.period_us, PERIOD_US_MIN..=PERIOD_US_MAX)
                .text("µs/step");
            if ui.add(slider).changed() {
                self.link.send(HostMsg::SetPeriod(self.period_us));
            }
        });

        match self.status {
            Some(st) if st.running => {
                // Six steps per electrical revolution
                let f_el = 1e6 / (6.0 * st.period_us as f32);
                ui.label(format!("Running | {} µs/step | {f_el:.0} Hz electrical", st.period_us));
            }
            Some(_) => {
                ui.label("Stopped");
            }
            None => {
                ui.label("No status received yet");
            }
        }
    }
}
