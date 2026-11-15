use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use protocol::{Calibration, DeviceMsg, HostMsg, SampleBlock, Status};

pub enum LinkEvent {
    Connected,
    Disconnected(String),
    Calibration(Calibration),
    Samples(SampleBlock),
    Status(Status),
    Stats { blocks_per_s: f32, lost: u64 },
}

/// Handle to a link thread. Dropping it stops the thread and closes the port.
pub struct Link {
    pub events: Receiver<LinkEvent>,
    cmds: Sender<HostMsg>,
}

impl Link {
    /// Queues a command for the device. Silently dropped if the link is closed.
    pub fn send(&self, msg: HostMsg) {
        let _ = self.cmds.send(msg);
    }
}

pub fn spawn(port: String, ctx: egui::Context) -> Link {
    let (tx, rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel();

    thread::spawn(move || {
        let reason = run(&port, &tx, &cmd_rx, &ctx);
        let _ = tx.send(LinkEvent::Disconnected(reason));
        ctx.request_repaint();
    });

    Link {
        events: rx,
        cmds: cmd_tx,
    }
}

/// Reads from the port until an error occurs or the Link is dropped.
fn run(
    port_name: &str,
    tx: &Sender<LinkEvent>,
    cmds: &Receiver<HostMsg>,
    ctx: &egui::Context,
) -> String {
    let mut port = match serialport::new(port_name, 115200)
        .timeout(Duration::from_millis(100))
        .open()
    {
        Ok(p) => p,
        Err(e) => return format!("open failed: {e}"),
    };
    port.clear(serialport::ClearBuffer::Input).ok();

    // Fails once the Link (receiver) is dropped
    let send = |ev: LinkEvent| -> bool {
        let ok = tx.send(ev).is_ok();
        ctx.request_repaint();
        ok
    };

    if !send(LinkEvent::Connected) {
        return "closed".into();
    }

    let mut buf: Vec<u8> = Vec::new();
    let mut read_buf = [0u8; 256];
    let mut stats = Stats::new();

    loop {
        for msg in cmds.try_iter() {
            let frame = postcard::to_stdvec_cobs(&msg).expect("HostMsg always serializes");
            if let Err(e) = port.write_all(&frame) {
                return format!("write failed: {e}");
            }
        }

        match port.read(&mut read_buf) {
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                while let Some(end) = buf.iter().position(|&b| b == 0) {
                    let mut frame: Vec<u8> = buf.drain(..=end).collect();

                    let ev = match postcard::from_bytes_cobs::<DeviceMsg>(&mut frame) {
                        Ok(DeviceMsg::Calibration(c)) => LinkEvent::Calibration(c),
                        Ok(DeviceMsg::Status(st)) => LinkEvent::Status(st),
                        Ok(DeviceMsg::CurrentSamples(block)) => {
                            stats.track(block.seq);
                            LinkEvent::Samples(block)
                        }
                        // Partial frame after joining mid-stream
                        Err(_) => continue,
                    };
                    if !send(ev) {
                        return "closed".into();
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => {}
            Err(e) => return format!("read failed: {e}"),
        }

        if let Some(ev) = stats.report() {
            if !send(ev) {
                return "closed".into();
            }
        }
    }
}

struct Stats {
    last_seq: Option<u32>,
    blocks: u32,
    lost: u64,
    since: Instant,
}

impl Stats {
    fn new() -> Self {
        Self {
            last_seq: None,
            blocks: 0,
            lost: 0,
            since: Instant::now(),
        }
    }

    fn track(&mut self, seq: u32) {
        if let Some(last) = self.last_seq {
            // seq going backwards means the device restarted, not a loss
            if seq > last {
                self.lost += (seq - last - 1) as u64;
            }
        }
        self.last_seq = Some(seq);
        self.blocks += 1;
    }

    /// Returns a stats event once per second.
    fn report(&mut self) -> Option<LinkEvent> {
        let elapsed = self.since.elapsed();
        if elapsed < Duration::from_secs(1) {
            return None;
        }
        let blocks_per_s = self.blocks as f32 / elapsed.as_secs_f32();
        self.blocks = 0;
        self.since = Instant::now();
        Some(LinkEvent::Stats {
            blocks_per_s,
            lost: self.lost,
        })
    }
}
