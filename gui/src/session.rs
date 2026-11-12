use eframe::egui;
use protocol::{BLOCK_LEN, Calibration, SampleBlock};

use crate::link::{Link, LinkEvent};

pub struct Session {
    link: Link,
    connected: bool,
    closed: Option<String>,
    calibration: Option<Calibration>,
    stats: Option<(f32, u64)>,
    last_block: Option<SampleBlock>,
}

impl Session {
    pub fn new(link: Link) -> Self {
        Self {
            link,
            connected: false,
            closed: None,
            calibration: None,
            stats: None,
            last_block: None,
        }
    }

    /// Applies all pending events from the link thread.
    pub fn poll(&mut self) {
        for ev in self.link.events.try_iter() {
            match ev {
                LinkEvent::Connected => self.connected = true,
                LinkEvent::Disconnected(reason) => self.closed = Some(reason),
                LinkEvent::Calibration(c) => self.calibration = Some(c),
                LinkEvent::Samples(block) => self.last_block = Some(block),
                LinkEvent::Stats { blocks_per_s, lost } => self.stats = Some((blocks_per_s, lost)),
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

        match &self.calibration {
            Some(c) => {
                let (bias_a, bias_c) = c.bias_mv();
                ui.label(format!("Bias: A {bias_a:.1} mV, C {bias_c:.1} mV"));
            }
            None => {
                ui.label("No calibration received yet (reset the board)");
            }
        }

        if let Some((rate, lost)) = self.stats {
            ui.label(format!(
                "{rate:.0} blocks/s | {:.0} samples/s | lost blocks: {lost}",
                rate * BLOCK_LEN as f32
            ));
        }

        if let (Some(c), Some(block)) = (&self.calibration, &self.last_block) {
            let s = block.samples[BLOCK_LEN - 1];
            let (i_a, i_c) = c.to_amps(s.i_a, s.i_c);
            ui.label(format!(
                "Step {} | I_a {i_a:.2} A | I_b {:.2} A | I_c {i_c:.2} A",
                s.step,
                -(i_a + i_c)
            ));
        }
    }
}
