use protocol::{Calibration, DeviceMsg};
use std::time::Duration;

fn main() {
    let port_name = find_port().expect("TractionHub not found. Is it connected?");
    println!("Connecting to {port_name}");

    let mut port = serialport::new(&port_name, 115200)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Failed to open port");

    let mut buf: Vec<u8> = Vec::new();
    let mut read_buf = [0u8; 64];
    let mut cal: Option<Calibration> = None;

    loop {
        match port.read(&mut read_buf) {
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                while let Some(end) = buf.iter().position(|&b| b == 0) {
                    let mut frame: Vec<u8> = buf.drain(..=end).collect();

                    match postcard::from_bytes_cobs::<DeviceMsg>(&mut frame) {
                        Ok(DeviceMsg::Calibration(c)) => {
                            let (bias_a, bias_c) = c.bias_mv();
                            println!("calibration: bias_a={bias_a:.1} mV bias_c={bias_c:.1} mV");
                            cal = Some(c);
                        }
                        Ok(DeviceMsg::Telemetry(t)) => match &cal {
                            Some(c) => {
                                let (i_a, i_c) = c.to_amps(t.i_a, t.i_c);
                                println!("step={} i_a={i_a:6.2} A i_c={i_c:6.2} A", t.step);
                            }
                            None => println!("step={} i_a={} i_c={} (raw)", t.step, t.i_a, t.i_c),
                        },
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
