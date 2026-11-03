use core::cell::RefCell;

use embassy_sync::blocking_mutex::Mutex;
use protocol::{Sample, SampleBlock, BLOCK_LEN};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

static CHANNEL: Channel<CriticalSectionRawMutex, SampleBlock, 8> = Channel::new();

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
            let _ = CHANNEL.try_send(self.block);
            self.seq_num += 1;
            self.sample_num = 0;
        }
    }
}

pub fn record(sample: Sample) {
    STREAM.lock(|s| s.borrow_mut().record(sample));
}

pub async fn next() -> SampleBlock {
    CHANNEL.receive().await
}

pub fn clear() {
    CHANNEL.clear();
}
