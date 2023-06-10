use std::time::Instant;

pub struct FpsCounter {
    cache: [f64; Self::LEN],
    head: usize,
    inst: Instant,
}

impl FpsCounter {
    const LEN: usize = 8;
    pub fn new() -> Self {
        Self {
            cache: [0.; 8],
            head: 0,
            inst: Instant::now(),
        }
    }

    pub fn record(&mut self) -> f64 {
        let curr = self.head;
        self.head = (self.head + 1) % Self::LEN;
        self.cache[curr] = self.inst.elapsed().as_secs_f64();
        self.inst = Instant::now();
        self.cache.iter().sum::<f64>() / Self::LEN as f64
    }
}
