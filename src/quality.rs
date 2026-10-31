//! Decoder-independent, separable antialiasing weights. No pixel allocation.

use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
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
            Filter::Area => ((f64::from(index) + 0.5).min(center + radius)
                - (f64::from(index) - 0.5).max(center - radius))
            .max(0.0),
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
    // Lanczos sinc zero crossings can make the lowest *nonzero* tap
    // nonmonotonic. The first/last destination locations do not necessarily
    // bound every source tap needed in the interior of an output ROI.
    let end = start.checked_add(count).ok_or(Error::GeometryOverflow)?;
    let mut first_source = u32::MAX;
    let mut last_source = 0_u32;
    for destination in start..end {
        let weights = taps(input, output, destination, filter)?;
        let begin = weights.first().ok_or(Error::InvalidDimensions)?.index;
        let finish = weights.last().ok_or(Error::InvalidDimensions)?.index;
        first_source = first_source.min(begin);
        last_source = last_source.max(finish);
    }
    Ok((first_source, last_source - first_source + 1))
}
