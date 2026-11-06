use crate::{Error, MAX_LEDS};

const MAX_ZONES: u16 = 256;
const MAX_MATRIX: u64 = 1 << 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub name: String,
    pub start: u32,
    pub leds: u32,
    pub matrix: Option<(u32, u32, Vec<u32>)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mode {
    pub index: i32,
    pub name: String,
    pub data: Vec<u8>,
    pub per_led: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    pub index: u32,
    pub name: String,
    pub key: String,
    pub serial: String,
    pub location: String,
    pub kind: &'static str,
    pub leds: u32,
    pub zones: Vec<Zone>,
    pub mode: Option<Mode>,
    pub colors: Vec<[u8; 3]>,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).filter(|end| *end <= self.data.len()).ok_or(Error::Broken)?;
        let found = &self.data[self.at..end];
        self.at = end;
        Ok(found)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| Error::Broken)?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| Error::Broken)?))
    }

    fn text(&mut self) -> Result<String, Error> {
        let n = usize::from(self.u16()?);
        let bytes = self.take(n)?;
        let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        Ok(String::from_utf8_lossy(bytes).chars().filter(|c| !c.is_control()).take(128).collect())
    }

    fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.take(n).map(drop)
    }
}

fn modes(r: &mut Reader<'_>, version: u32) -> Result<Option<Mode>, Error> {
    let count = r.u16()?;
    let active = r.u32()? as i32;
    let mut found = None;
    for index in 0..i32::from(count) {
        let start = r.at;
        let name = r.text()?;
        r.skip(16 + if version >= 3 { 8 } else { 0 })?;
        r.skip(12 + if version >= 3 { 4 } else { 0 })?;
        r.u32()?;
        let per_led = r.u32()? == 1;
        let colors = usize::from(r.u16()?);
        r.skip(4 * colors)?;
        if index == active {
            found = Some(Mode { index, name, data: r.data[start..r.at].to_vec(), per_led });
        }
    }
    Ok(found)
}

fn colors(r: &mut Reader<'_>, leds: u32) -> Result<Vec<[u8; 3]>, Error> {
    for _ in 0..leds {
        r.text()?;
        r.u32()?;
    }
    let count = r.u16()?;
    (0..count).map(|_| r.take(4).map(|c| [c[0], c[1], c[2]])).collect()
}

fn zone(r: &mut Reader<'_>, version: u32, start: u32) -> Result<Zone, Error> {
    let name = r.text()?;
    r.skip(12)?;
    let leds = r.u32()?;
    let matrix = match r.u16()? {
        0 => None,
        _ => {
            let (h, w) = (r.u32()?, r.u32()?);
            let cells = u64::from(h) * u64::from(w);
            if cells == 0 || cells > MAX_MATRIX {
                return Err(Error::Broken);
            }
            let map = (0..cells).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
            Some((w, h, map))
        }
    };
    if version >= 4 {
        for _ in 0..r.u16()? {
            r.text()?;
            r.skip(12)?;
        }
    }
    Ok(Zone { name, start, leds, matrix })
}

pub fn controller(index: u32, data: &[u8], version: u32) -> Result<Device, Error> {
    let mut r = Reader { data, at: 0 };
    r.u32()?;
    let kind = crate::kind(r.u32()? as i32);
    let name = r.text()?;
    let texts = if version >= 1 { 3 } else { 2 };
    (0..texts).try_for_each(|_| r.text().map(drop))?;
    let (serial, location) = (r.text()?, r.text()?);
    let mode = modes(&mut r, version)?;
    let count = r.u16()?;
    if count > MAX_ZONES {
        return Err(Error::Broken);
    }
    let mut zones = Vec::new();
    let mut start = 0u32;
    for _ in 0..count {
        let z = zone(&mut r, version, start)?;
        start = start.checked_add(z.leds).ok_or(Error::Broken)?;
        zones.push(z);
    }
    let leds = u32::from(r.u16()?);
    if leds as usize > MAX_LEDS || start > leds {
        return Err(Error::Broken);
    }
    let colors = colors(&mut r, leds).unwrap_or_default();
    let key = name.clone();
    Ok(Device { index, name, key, serial, location, kind, leds, zones, mode, colors })
}

pub fn keyed(devices: &mut [Device]) {
    let names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
    for (i, name) in names.iter().enumerate() {
        let twins: Vec<usize> = (0..names.len()).filter(|j| names[*j] == *name).collect();
        if twins.len() < 2 {
            continue;
        }
        let unique = |part: fn(&Device) -> &str| {
            let mine = part(&devices[i]);
            (!mine.is_empty() && twins.iter().filter(|j| part(&devices[**j]) == mine).count() == 1)
                .then(|| mine.to_owned())
        };
        let order = twins.iter().position(|j| *j == i).unwrap_or_default() + 1;
        let tail = unique(|d| &d.serial).or_else(|| unique(|d| &d.location)).unwrap_or(order.to_string());
        let tail: String = tail.chars().filter(|c| *c != '#').take(48).collect();
        let kept: String = name.chars().take(128 - 3 - tail.chars().count()).collect();
        devices[i].key = format!("{kept} ({tail})");
    }
}
