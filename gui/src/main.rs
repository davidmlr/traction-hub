use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct Telemetry {
    val_1: u16,
    val_2: u16,
}

fn main() {
    let port_name = find_port().expect("TractionHub not found. Is it connected?");
    println!("Connecting to {port_name}");

    let mut port = serialport::new(&port_name, 115200)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Failed to open port");

    let mut buf: Vec<u8> = Vec::new();
    let mut read_buf = [0u8; 64];

    loop {
        match port.read(&mut read_buf) {
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                while let Some(end) = buf.iter().position(|&b| b == 0) {
                    let mut frame: Vec<u8> = buf.drain(..=end).collect();

                    match postcard::from_bytes_cobs::<Telemetry>(&mut frame) {
                        Ok(t) => println!("val_1={} val_2={}", t.val_1, t.val_2),
                        Err(e) => eprintln!("decode error: {e}"),
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => eprintln!("read error: {e}"),
        }
    }
}

fn find_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;

    // Try to find by VID/PID
    for p in &ports {
        if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
            if info.vid == 0xdead && info.pid == 0xbeef {
                return Some(p.port_name.clone());
            }
        }
    }

    None
}
