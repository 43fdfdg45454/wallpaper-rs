use std::io::Result;

#[cfg(not(target_os = "android"))]
pub(crate) const MAX_MESSAGE: usize = 4 << 20;

pub(crate) struct Wire(pub(crate) Vec<u8>);

impl Wire {
    pub(crate) fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }

    pub(crate) fn u16(&mut self, v: usize) -> &mut Self {
        self.0.extend((v.min(u16::MAX as usize) as u16).to_le_bytes());
        self
    }

    pub(crate) fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }

    pub(crate) fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }

    pub(crate) fn f32s(&mut self, v: &[f32]) -> &mut Self {
        v.iter().for_each(|x| self.0.extend(x.to_le_bytes()));
        self
    }

    pub(crate) fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }

    pub(crate) fn flag(&mut self, on: bool) -> bool {
        self.0.push(u8::from(on));
        on
    }

    pub(crate) fn str(&mut self, v: &str) -> &mut Self {
        self.0.extend((v.len() as u32).to_le_bytes());
        self.0.extend(v.as_bytes());
        self
    }

    pub(crate) fn opt(&mut self, v: Option<&str>) -> &mut Self {
        if self.flag(v.is_some()) {
            self.str(v.unwrap_or_default());
        }
        self
    }
}

pub(crate) struct Cursor<'a>(pub(crate) &'a [u8]);

fn short() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "a script message ended early")
}

impl<'a> Cursor<'a> {
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n).ok_or_else(short)?;
        self.0 = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N).map(|b| b.try_into().unwrap_or([0; N]))
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        self.array::<1>().map(|[b]| b)
    }

    pub(crate) fn flag(&mut self) -> Result<bool> {
        self.u8().map(|b| b != 0)
    }

    pub(crate) fn u16(&mut self) -> Result<usize> {
        self.array().map(u16::from_le_bytes).map(usize::from)
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub(crate) fn f32(&mut self) -> Result<f32> {
        self.array().map(f32::from_le_bytes)
    }

    pub(crate) fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }

    pub(crate) fn f64(&mut self) -> Result<f64> {
        self.array().map(f64::from_le_bytes)
    }

    pub(crate) fn str(&mut self) -> Result<&'a str> {
        let n = self.u32()? as usize;
        std::str::from_utf8(self.take(n)?).map_err(std::io::Error::other)
    }

    pub(crate) fn opt(&mut self) -> Result<Option<&'a str>> {
        if self.flag()? { self.str().map(Some) } else { Ok(None) }
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn send(out: &mut impl std::io::Write, tag: u8, body: &[u8]) -> Result<()> {
    out.write_all(&((body.len() + 1) as u32).to_le_bytes())?;
    out.write_all(&[tag])?;
    out.write_all(body)?;
    out.flush()
}

#[cfg(not(target_os = "android"))]
pub(crate) fn receive(input: &mut impl std::io::Read, into: &mut Vec<u8>) -> Result<Option<u8>> {
    let mut size = [0; 4];
    match input.read_exact(&mut size) {
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    }
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_MESSAGE {
        return Err(std::io::Error::other("a script message is too large"));
    }
    let mut tag = [0];
    input.read_exact(&mut tag)?;
    into.resize(size - 1, 0);
    input.read_exact(into)?;
    Ok(Some(tag[0]))
}
