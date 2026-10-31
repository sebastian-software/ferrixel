//! Decoder-independent, separable antialiasing weights. No pixel allocation.

use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Filter {
    Area,
    Lanczos3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tap {
    pub index: u32,
    pub weight: f64,
}

pub(crate) fn taps(input: u32, output: u32, at: u32, filter: Filter) -> Result<Vec<Tap>, Error> {
    if input == 0 || output == 0 || at >= output {
        return Err(Error::InvalidDimensions);
    }
    let ratio = f64::from(input) / f64::from(output);
    let center = (f64::from(at) + 0.5) * ratio - 0.5;
    let scale = ratio.max(1.0);
    let radius = match filter {
        Filter::Area => scale * 0.5,
        Filter::Lanczos3 => scale * 3.0,
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let first = (center - radius - 1.0).floor().max(0.0) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let last = (center + radius + 1.0).ceil().min(f64::from(input - 1)) as u32;
    let mut weights = Vec::new();
    let mut sum = 0.0;
    for index in first..=last {
        let distance = f64::from(index) - center;
        let weight = match filter {
            Filter::Area => {
                ( (f64::from(index) + 0.5).min(center + radius)
                    - (f64::from(index) - 0.5).max(center - radius)).max(0.0)
            }
            Filter::Lanczos3 => {
                let d = distance / scale;
                if d.abs() < 3.0 {
                    sinc(d) * sinc(d / 3.0)
                } else {
                    0.0
                }
            }
        };
        if weight.abs() > f64::EPSILON {
            weights.push(Tap { index, weight });
            sum += weight;
        }
    }
    if weights.is_empty() || !sum.is_finite() || sum.abs() < f64::EPSILON {
        return Err(Error::UnsupportedPixelOperation);
    }
    for tap in &mut weights {
        tap.weight /= sum;
    }
    Ok(weights)
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1.0e-12 {
        1.0
    } else {
        let p = x * std::f64::consts::PI;
        p.sin() / p
    }
}

pub(crate) fn footprint(
    input: u32,
    output: u32,
    start: u32,
    count: u32,
    filter: Filter,
) -> Result<(u32, u32), Error> {
    if count == 0 || start.checked_add(count).is_none_or(|end| end > output) {
        return Err(Error::OutOfBounds);
    }
    let first = taps(input, output, start, filter)?;
    let last = taps(input, output, start + count - 1, filter)?;
    let begin = first.first().ok_or(Error::InvalidDimensions)?.index;
    let end = last.last().ok_or(Error::InvalidDimensions)?.index;
    Ok((begin, end - begin + 1))
}
