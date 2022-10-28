use std::io::{self, Read, Write};

pub struct ReadWrite<R, W> {
    r: R,
    w: W,
}

impl<R: Read, W: Write> ReadWrite<R, W> {
    pub fn new(r: R, w: W) -> ReadWrite<R, W> {
        ReadWrite { r, w }
    }
}

impl<R: Read, W> Read for ReadWrite<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.r.read(buf)
    }
}

impl<R, W: Write> Write for ReadWrite<R, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

impl<R: Read, W: Write> From<(R, W)> for ReadWrite<R, W> {
    fn from((r, w): (R, W)) -> ReadWrite<R, W> {
        ReadWrite { r, w }
    }
}
