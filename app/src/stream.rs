//! Sample stream from the ADC ISR to the USB task.
//!
//! The ISR writes samples directly into a slot of a zero-copy channel. When a block is full
//! it only marks the slot as done, so the ISR never copies a whole block.

use core::cell::RefCell;
use core::sync::atomic::{AtomicU16, Ordering::Relaxed};

use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use embassy_sync::zerocopy_channel::{Channel, Receiver, Sender};
use protocol::{Sample, SampleBlock, BLOCK_LEN};
use static_cell::StaticCell;

const SLOTS: usize = 8;

pub type BlockReceiver = Receiver<'static, CriticalSectionRawMutex, SampleBlock>;
type BlockSender = Sender<'static, CriticalSectionRawMutex, SampleBlock>;

// Max ISR timing in TIM1 ticks since the last full block
static ISR_RUN_MAX: AtomicU16 = AtomicU16::new(0);
static ISR_END_MAX: AtomicU16 = AtomicU16::new(0);

static STREAM: Mutex<CriticalSectionRawMutex, RefCell<Option<Stream>>> =
    Mutex::new(RefCell::new(None));

struct Stream {
    sender: BlockSender,
    sample_num: usize,
    seq_num: u32,
    /// Slot the current block is written to
    slot: *const SampleBlock,
    /// False if samples of the current block were lost (channel full or cleared)
    valid: bool,
}

// The raw slot pointer is only compared, never dereferenced
unsafe impl Send for Stream {}

impl Stream {
    fn record(&mut self, sample: Sample) {
        let n = self.sample_num;
        match self.sender.try_send() {
            Some(block) => {
                let ptr = block as *const SampleBlock;
                if n != 0 && ptr != self.slot {
                    // Got a different slot mid-block: its first samples are stale
                    self.valid = false;
                }
                self.slot = ptr;
                block.samples[n] = sample;

                if n == BLOCK_LEN - 1 {
                    block.seq = self.seq_num;
                    // Covers the ISR runs up to the previous sample, the current one is still running
                    block.isr_run_max_ticks = ISR_RUN_MAX.swap(0, Relaxed);
                    block.isr_end_max_ticks = ISR_END_MAX.swap(0, Relaxed);
                    if self.valid {
                        self.sender.send_done();
                    }
                }
            }
            // Channel full, this block is lost. The host sees it as a gap in seq.
            None => self.valid = false,
        }

        self.sample_num += 1;
        if self.sample_num == BLOCK_LEN {
            self.sample_num = 0;
            self.seq_num = self.seq_num.wrapping_add(1);
            self.valid = true;
        }
    }
}

/// Creates the channel. Must be called once before the ADC interrupt is enabled.
pub fn init() -> BlockReceiver {
    static SLOTS_BUF: StaticCell<[SampleBlock; SLOTS]> = StaticCell::new();
    static CHANNEL: StaticCell<Channel<'static, CriticalSectionRawMutex, SampleBlock>> =
        StaticCell::new();

    let buf = SLOTS_BUF.init([SampleBlock::new(); SLOTS]);
    let channel = CHANNEL.init(Channel::new(buf));
    let (sender, receiver) = channel.split();

    STREAM.lock(|s| {
        s.replace(Some(Stream {
            sender,
            sample_num: 0,
            seq_num: 0,
            slot: core::ptr::null(),
            valid: true,
        }))
    });
    receiver
}

/// Called from the ADC ISR for every sample.
pub fn record(sample: Sample) {
    STREAM.lock(|s| {
        if let Some(stream) = s.borrow_mut().as_mut() {
            stream.record(sample);
        }
    });
}

/// Records the ISR run time (entry to exit) and end time (ADC trigger to exit) in TIM1 ticks.
pub fn record_isr_time(run: u32, end: u32) {
    ISR_RUN_MAX.fetch_max(run.min(u16::MAX as u32) as u16, Relaxed);
    ISR_END_MAX.fetch_max(end.min(u16::MAX as u32) as u16, Relaxed);
}
