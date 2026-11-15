use core::cell::RefCell;
use core::sync::atomic::{AtomicU16, Ordering::Relaxed};

use embassy_sync::blocking_mutex::Mutex;
use protocol::{Sample, SampleBlock, BLOCK_LEN};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

static CHANNEL: Channel<CriticalSectionRawMutex, SampleBlock, 8> = Channel::new();

// Max ISR timing in TIM1 ticks since the last full block
static ISR_RUN_MAX: AtomicU16 = AtomicU16::new(0);
static ISR_END_MAX: AtomicU16 = AtomicU16::new(0);

static STREAM: Mutex<CriticalSectionRawMutex, RefCell<Stream>> =
    Mutex::new(RefCell::new(Stream::new()));

pub struct Stream {
    sample_num: usize,
    seq_num: u32,
    block: SampleBlock,
}

impl Stream {
    const fn new() -> Self {
        Stream {
            sample_num: 0,
            seq_num: 0,
            block: SampleBlock::new(),
        }
    }
    fn record(&mut self, sample: Sample) {
        self.block.samples[self.sample_num] = sample;

        self.sample_num += 1;
        if self.sample_num == BLOCK_LEN {
            self.block.seq = self.seq_num;
            // Covers the ISR runs up to the previous sample, the current one is still running
            self.block.isr_run_max_ticks = ISR_RUN_MAX.swap(0, Relaxed);
            self.block.isr_end_max_ticks = ISR_END_MAX.swap(0, Relaxed);
            let _ = CHANNEL.try_send(self.block);
            self.seq_num += 1;
            self.sample_num = 0;
        }
    }
}

pub fn record(sample: Sample) {
    STREAM.lock(|s| s.borrow_mut().record(sample));
}

/// Records the ISR run time (entry to exit) and end time (ADC trigger to exit) in TIM1 ticks.
pub fn record_isr_time(run: u32, end: u32) {
    ISR_RUN_MAX.fetch_max(run.min(u16::MAX as u32) as u16, Relaxed);
    ISR_END_MAX.fetch_max(end.min(u16::MAX as u32) as u16, Relaxed);
}

pub async fn next() -> SampleBlock {
    CHANNEL.receive().await
}

pub fn clear() {
    CHANNEL.clear();
}
