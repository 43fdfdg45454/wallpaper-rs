use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sun {
    Rises { rise: DateTime<Utc>, set: DateTime<Utc> },
    PolarDay,
    PolarNight,
}

impl Location {
    pub fn is_valid(&self) -> bool {
        (-90.0..=90.0).contains(&self.latitude) && (-180.0..=180.0).contains(&self.longitude)
    }

    pub fn sun(&self, date: NaiveDate) -> Sun {
        let jd = 1721424.5 + f64::from(date.num_days_from_ce());
        let event = |sign: f64| -> Result<f64, Sun> {
            let first = self.event(jd, 720.0 - 4.0 * self.longitude, sign)?;
            Ok(self.event(jd, first, sign).unwrap_or(first))
        };
        match (event(1.0), event(-1.0)) {
            (Ok(rise), Ok(set)) => Sun::Rises { rise: at(date, rise), set: at(date, set) },
            (Err(polar), _) | (_, Err(polar)) => polar,
        }
    }

    pub fn is_day(&self, at: DateTime<Utc>) -> bool {
        (-1..=1)
            .filter_map(|d| self.daylight(at.date_naive() + TimeDelta::days(d)))
            .any(|(a, b)| a <= at && at < b)
    }

    pub fn next_sun_change(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        (-1..=367)
            .filter_map(|d| self.daylight(after.date_naive() + TimeDelta::days(d)))
            .flat_map(|(a, b)| [a, b])
            .filter(|t| *t > after && self.is_day(*t) != self.is_day(*t - TimeDelta::seconds(1)))
            .take(4)
            .min()
    }

    fn daylight(&self, date: NaiveDate) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        match self.sun(date) {
            Sun::Rises { rise, set } => Some((rise, set)),
            Sun::PolarDay => {
                let noon = at(date, 720.0 - 4.0 * self.longitude);
                Some((noon - TimeDelta::hours(13), noon + TimeDelta::hours(13)))
            }
            Sun::PolarNight => None,
        }
    }

    fn event(&self, jd: f64, minute: f64, sign: f64) -> Result<f64, Sun> {
        let (declination, equation) = solar(jd + minute / 1440.0);
        let lat = self.latitude.clamp(-89.99, 89.99).to_radians();
        let zenith = 90.833_f64.to_radians();
        match zenith.cos() / (lat.cos() * declination.cos()) - lat.tan() * declination.tan() {
            c if c > 1.0 => Err(Sun::PolarNight),
            c if c < -1.0 => Err(Sun::PolarDay),
            c => Ok(720.0 - 4.0 * (self.longitude + sign * c.acos().to_degrees()) - equation),
        }
    }
}

fn at(date: NaiveDate, minutes: f64) -> DateTime<Utc> {
    date.and_time(NaiveTime::MIN).and_utc() + TimeDelta::seconds((minutes * 60.0).round() as i64)
}

fn solar(jd: f64) -> (f64, f64) {
    let t = (jd - 2451545.0) / 36525.0;
    let l0 = (280.46646 + t * (36000.76983 + t * 0.0003032)).rem_euclid(360.0).to_radians();
    let m = (357.52911 + t * (35999.05029 - 0.0001537 * t)).to_radians();
    let e = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);
    let center = m.sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + (2.0 * m).sin() * (0.019993 - 0.000101 * t)
        + (3.0 * m).sin() * 0.000289;
    let omega = (125.04 - 1934.136 * t).to_radians();
    let lambda = (l0.to_degrees() + center - 0.00569 - 0.00478 * omega.sin()).to_radians();
    let mean_obliquity = 23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0) / 60.0;
    let obliquity = (mean_obliquity + 0.00256 * omega.cos()).to_radians();
    let declination = (obliquity.sin() * lambda.sin()).asin();
    let y = (obliquity / 2.0).tan().powi(2);
    let equation = y * (2.0 * l0).sin() - 2.0 * e * m.sin() + 4.0 * e * y * m.sin() * (2.0 * l0).cos()
        - 0.5 * y * y * (4.0 * l0).sin()
        - 1.25 * e * e * (2.0 * m).sin();
    (declination, 4.0 * equation.to_degrees())
}
