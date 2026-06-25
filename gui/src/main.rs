use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct Telemetry {
    timestamp_ms: u32,
    vbus_v: f32,
    ia_a: f32,
    ib_a: f32,
    ic_a: f32,
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

                while let Some(pos) = buf.iter().position(|&b| b == 0x00) {
                    let mut frame: Vec<u8> = buf.drain(..=pos).collect();
                    match postcard::from_bytes_cobs::<Telemetry>(&mut frame) {
                        Ok(t) => println!(
                            "t={:>8}ms  Vbus={:.2}V  Ia={:+.3}A  Ib={:+.3}A  Ic={:+.3}A",
                            t.timestamp_ms, t.vbus_v, t.ia_a, t.ib_a, t.ic_a
                        ),
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
            if info.vid == 0xc0de && info.pid == 0xcafe {
                return Some(p.port_name.clone());
            }
        }
    }

    None
}
