use wprs_core::message::{Message, said};

pub const MAX_LUT_SIZE: u32 = 65;
pub const MAX_LUT_BYTES: usize = 16 << 20;
pub const MAX_LUT_TOTAL: usize = 64 << 20;

#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
    pub size: u32,
    pub data: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{}", self.message())]
pub enum LutError {
    TooLarge,
    Text,
    Line(usize),
    Size,
    Count(usize, usize),
}

impl LutError {
    pub fn message(&self) -> Message {
        match self {
            LutError::TooLarge => said("scene.lut_large", "the LUT is larger than 16 MiB", &[]),
            LutError::Text => said("scene.lut_text", "the LUT is not UTF-8 text", &[]),
            LutError::Line(n) => said("scene.lut_line", "line %1 of the LUT is not valid", &[n]),
            LutError::Size => said(
                "scene.lut_size",
                "the LUT needs LUT_3D_SIZE between 2 and %1 before its data",
                &[&MAX_LUT_SIZE],
            ),
            LutError::Count(n, m) => said("scene.lut_count", "the LUT has %1 entries, expected %2", &[n, m]),
        }
    }
}

impl Lut {
    pub fn parse(bytes: &[u8]) -> Result<Lut, LutError> {
        if bytes.len() > MAX_LUT_BYTES {
            return Err(LutError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| LutError::Text)?;
        let (mut size, mut data) = (0u32, Vec::new());
        for (i, line) in text.lines().enumerate() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let numbers = |w: &[&str]| -> Option<[f32; 3]> {
                let v: Vec<f32> =
                    w.iter().filter_map(|x| x.parse().ok()).filter(|x: &f32| x.is_finite()).collect();
                v.try_into().ok()
            };
            match words.as_slice() {
                [] | ["TITLE", ..] => {}
                [first, ..] if first.starts_with('#') => {}
                ["LUT_3D_SIZE", n] if size == 0 => {
                    size = n.parse().ok().filter(|n| (2..=MAX_LUT_SIZE).contains(n)).ok_or(LutError::Size)?;
                    data.reserve_exact(size.pow(3) as usize);
                }
                ["DOMAIN_MIN", rest @ ..] if numbers(rest) == Some([0.0; 3]) => {}
                ["DOMAIN_MAX", rest @ ..] if numbers(rest) == Some([1.0; 3]) => {}
                [_, _, _] if size == 0 && numbers(&words).is_some() => return Err(LutError::Size),
                [_, _, _] if data.len() < size.pow(3) as usize => {
                    data.push(numbers(&words).ok_or(LutError::Line(i + 1))?);
                }
                [_, _, _] => return Err(LutError::Count(data.len() + 1, size.pow(3) as usize)),
                _ => return Err(LutError::Line(i + 1)),
            }
        }
        match size {
            0 => Err(LutError::Size),
            n if data.len() != n.pow(3) as usize => Err(LutError::Count(data.len(), n.pow(3) as usize)),
            _ => Ok(Lut { size, data }),
        }
    }
}
