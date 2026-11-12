use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use eframe::egui;

const VID: u16 = 0xdead;
const PID: u16 = 0xbeef;

#[derive(Clone, PartialEq)]
pub struct BoardInfo {
    pub port: String,
    pub serial: Option<String>,
}

impl BoardInfo {
    pub fn key(&self) -> String {
        self.serial.clone().unwrap_or_else(|| self.port.clone())
    }
}

pub fn spawn(ctx: egui::Context) -> Receiver<Vec<BoardInfo>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut last = Vec::new();
        loop {
            let boards = scan();
            if boards != last {
                if tx.send(boards.clone()).is_err() {
                    break;
                }
                ctx.request_repaint();
                last = boards;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    rx
}

fn scan() -> Vec<BoardInfo> {
    let Ok(ports) = serialport::available_ports() else {
        return Vec::new();
    };
    ports
        .into_iter()
        .filter_map(|p| match p.port_type {
            serialport::SerialPortType::UsbPort(info) if info.vid == VID && info.pid == PID => {
                Some(BoardInfo {
                    port: p.port_name,
                    serial: info.serial_number,
                })
            }
            _ => None,
        })
        .collect()
}
