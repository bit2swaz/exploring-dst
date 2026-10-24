// a fake clock
//
// a real clock moves by itself. this does not
// only the simulator moves it w/ `advance`. thus the time is the same on every run

pub struct Clock {
    now_ms: u64,
}

impl Clock {
    pub fn new() -> Clock {
        Clock { now_ms: 0 }
    }

    /// the current fake time in ms since the start
    pub fn now(&self) -> u64 {
        self.now_ms
    }

    /// move the time forward. this is the only way that time changes
    pub fn advance(&mut self, ms: u64) {
        self.now_ms += ms;
    }
}
