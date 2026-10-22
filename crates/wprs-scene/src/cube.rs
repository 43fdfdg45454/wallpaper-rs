pub const MAX_LUT_SIZE: u32 = 65;
pub const MAX_LUT_BYTES: usize = 16 << 20;
pub const MAX_LUT_TOTAL: usize = 64 << 20;

#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
    pub size: u32,
    pub data: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LutError {
    #[error("the LUT is larger than 16 MiB")]
    TooLarge,
    #[error("the LUT is not UTF-8 text")]
    Text,
    #[error("line {0} of the LUT is not valid")]
    Line(usize),
    #[error("the LUT needs LUT_3D_SIZE between 2 and {MAX_LUT_SIZE} before its data")]
    Size,
    #[error("the LUT has {0} entries, expected {1}")]
    Count(usize, usize),
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
