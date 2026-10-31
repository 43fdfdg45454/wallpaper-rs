use std::io::{self, Write};
use wprs_format::Fetch;

pub struct Dry;

impl Fetch for Dry {
    fn get(&self, _: &str, _: &str, _: u64) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

pub struct Progress<F, P>(pub F, pub P);

struct Counted<'a>(&'a mut dyn Write, &'a dyn Fn(u64), u64);

impl Write for Counted<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.0.write(buf)?;
        self.2 += written as u64;
        (self.1)(self.2);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<F: Fetch, P: Fn(u64)> Fetch for Progress<F, P> {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        self.0.get(base, path, max)
    }

    fn copy(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        self.0.copy(base, path, max, &mut Counted(out, &self.1, 0))
    }
}
