use super::Rgb;

pub const PLACE: &str = "rgb.place.";
pub const MAX_PLACES: usize = 256;
const SMALLEST: f32 = 0.001;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Flow {
    #[default]
    Right,
    Left,
    Down,
    Up,
}

impl Flow {
    pub const WORDS: [&str; 4] = ["right", "left", "down", "up"];

    pub fn as_str(self) -> &'static str {
        Self::WORDS[self as usize]
    }

    fn parse(word: &str) -> Option<Flow> {
        [Flow::Right, Flow::Left, Flow::Down, Flow::Up].into_iter().find(|f| f.as_str() == word)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    Off,
    At { rect: [f32; 4], flow: Flow },
}

impl Place {
    pub fn parse(text: &str) -> Result<Place, String> {
        let text = text.trim();
        if text == "off" {
            return Ok(Place::Off);
        }
        let wrong =
            || format!("{text:?} is not off or x y w h [{}] in parts of the picture", Flow::WORDS.join("|"));
        let words: Vec<&str> = text.split_whitespace().collect();
        if !(4..=5).contains(&words.len()) {
            return Err(wrong());
        }
        let numbers: Vec<f32> =
            words[..4].iter().map(|w| w.parse::<f32>()).collect::<Result<_, _>>().map_err(|_| wrong())?;
        let [x, y, w, h] = [numbers[0], numbers[1], numbers[2], numbers[3]];
        let inside =
            |at: f32, size: f32| (0.0..=1.0).contains(&at) && size >= SMALLEST && at + size <= 1.0 + 1e-4;
        if !numbers.iter().all(|v| v.is_finite()) || !inside(x, w) || !inside(y, h) {
            return Err(wrong());
        }
        let flow = words.get(4).map_or(Some(Flow::Right), |word| Flow::parse(word)).ok_or_else(wrong)?;
        let rect = [x, y, w.min(1.0 - x).max(SMALLEST), h.min(1.0 - y).max(SMALLEST)];
        Ok(Place::At { rect, flow })
    }

    pub fn text(&self) -> String {
        let round = |v: f32| (v * 1000.0).round() / 1000.0;
        match self {
            Place::Off => "off".into(),
            Place::At { rect, flow } => {
                let [x, y, w, h] = rect.map(round);
                format!("{x} {y} {w} {h} {}", flow.as_str())
            }
        }
    }
}

fn named(key: &str) -> bool {
    let (device, zone) = match key.rsplit_once('#') {
        Some((device, zone)) if zone.parse::<u16>().is_ok() => (device, Some(zone)),
        _ => (key, None),
    };
    let fine = |part: &str| (1..=128).contains(&part.chars().count()) && !part.chars().any(char::is_control);
    fine(device) && zone.is_none_or(|z| z.len() <= 5)
}

impl Rgb {
    pub fn set_place(&mut self, key: &str, value: &str) -> Result<(), String> {
        let key = key.trim();
        if !named(key) {
            return Err(format!("invalid device name in {PLACE}<device>[#<zone>]"));
        }
        if matches!(value.trim(), "" | "none") {
            self.places.remove(key);
            return Ok(());
        }
        let wrong = |e: String| format!("invalid value for {PLACE}{key}: {e}");
        let place = Place::parse(value).map_err(wrong)?;
        Place::parse(&place.text()).map_err(wrong)?;
        if !self.places.contains_key(key) && self.places.len() >= MAX_PLACES {
            return Err(format!("at most {MAX_PLACES} devices and zones can be placed"));
        }
        self.places.insert(key.into(), place.text());
        Ok(())
    }

    pub fn place(&self, device: &str) -> Option<Place> {
        self.places.get(device).and_then(|text| Place::parse(text).ok())
    }

    pub fn zone_place(&self, device: &str, zone: usize) -> Option<Place> {
        self.place(&format!("{device}#{zone}"))
    }

    pub(super) fn tidy_places(&mut self) {
        self.places.retain(|key, value| named(key) && Place::parse(value).is_ok());
        while self.places.len() > MAX_PLACES {
            self.places.pop_last();
        }
    }
}
