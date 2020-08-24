pub struct Uart {
    buffer: u8,
}

impl Uart {
    pub fn new() -> Self {
        Self {
            buffer: 0,
        }
    }
    
    pub fn store(&mut self, addr: u64, value: u8) {
        if (addr & 0b111) == 0 {
            // RHR
            self.buffer = value;
            print!("{}", self.buffer as char);
        } else if (addr & 0b111) == 0b101 {
            // LSR
        }
    }
}
