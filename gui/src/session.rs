use eframe::egui;
use protocol::{BLOCK_LEN, Calibration, Status};

use crate::link::{Link, LinkEvent};
use crate::scope::Scope;

pub struct Session {
    link: Link,
    connected: bool,
    closed: Option<String>,
    calibration: Option<Calibration>,
    stats: Option<(f32, u64)>,
    status: Option<Status>,
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

        if let Some(st) = self.status {
            let state = if st.running { "running" } else { "stopped" };
            ui.label(format!("Motor {state} | {} µs/step", st.period_us));
        }

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
}
