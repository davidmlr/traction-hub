use protocol::{BLOCK_LEN, Calibration, DeviceMsg, SampleBlock};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

// One sample per PWM period (20 kHz)
const SAMPLE_PERIOD_MS: f64 = 0.05;

fn main() {
    let path = std::env::args().nth(1).unwrap_or("capture.csv".into());
    let port_name = find_port().expect("TractionHub not found. Is it connected?");
    println!("Connecting to {port_name}, writing to {path}");

    let mut port = serialport::new(&port_name, 115200)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Failed to open port");
    port.clear(serialport::ClearBuffer::Input).ok();

    let mut csv = BufWriter::new(File::create(&path).expect("Failed to create csv"));
    writeln!(csv, "t_ms,seq,step,i_a,i_b,i_c").unwrap();

    let mut buf: Vec<u8> = Vec::new();
    let mut read_buf = [0u8; 256];
    let mut cal: Option<Calibration> = None;
    let mut stats = Stats::new();

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
                            stats.calibrated = true;
                        }
                        Ok(DeviceMsg::CurrentSamples(block)) => {
                            stats.track(block.seq);
                            // Without calibration the raw values can't be converted
                            if let Some(c) = &cal {
                                write_block(&mut csv, c, &block);
                            }
                        }
                        Err(e) => eprintln!("decode error: {e}"),
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => eprintln!("read error: {e}"),
        }

        if stats.report_due() {
            // Flush regularly, Ctrl+C would lose the buffered rows otherwise
            csv.flush().unwrap();
        }
    }
}

fn write_block(csv: &mut impl Write, cal: &Calibration, block: &SampleBlock) {
    for (i, s) in block.samples.iter().enumerate() {
        let index = block.seq as u64 * BLOCK_LEN as u64 + i as u64;
        let t_ms = index as f64 * SAMPLE_PERIOD_MS;
        let (i_a, i_c) = cal.to_amps(s.i_a, s.i_c);
        // Kirchhoff: the three phase currents sum to zero
        let i_b = -(i_a + i_c);
        writeln!(
            csv,
            "{t_ms:.3},{},{},{i_a:.3},{i_b:.3},{i_c:.3}",
            block.seq, s.step
        )
        .unwrap();
    }
}

struct Stats {
    last_seq: Option<u32>,
    blocks: u32,
    lost: u64,
    since: Instant,
    calibrated: bool,
}

impl Stats {
    fn new() -> Self {
        Self {
            last_seq: None,
            blocks: 0,
            lost: 0,
            since: Instant::now(),
            calibrated: false,
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

    /// Prints the stats once per second, returns true when it did.
    fn report_due(&mut self) -> bool {
        let elapsed = self.since.elapsed();
        if elapsed < Duration::from_secs(1) {
            return false;
        }
        let rate = self.blocks as f64 / elapsed.as_secs_f64();
        if !self.calibrated && self.blocks > 0 {
            println!("no calibration received yet, nothing is written (reset the board)");
        }
        println!(
            "{rate:.0} blocks/s | {:.0} samples/s | lost blocks: {}",
            rate * BLOCK_LEN as f64,
            self.lost
        );
        self.blocks = 0;
        self.since = Instant::now();
        true
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
