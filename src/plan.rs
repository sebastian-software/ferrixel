//! Backward, exact-geometry region demand for the first scalar experiment.
//!
//! The executor deliberately plans **one output scanline at a time**. A huge
//! downscale can require distant source scanlines; expanding one bounding
//! rectangle for the entire output would otherwise materialize the full input.
//! This is a proof of the dependency contract, not a final scheduling policy.

use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::{Graph, NodeId, NodeKind};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Step {
    pub node: NodeId,
    /// The precise output region this node must produce for the consumer.
    pub region: Rect,
}

pub(crate) fn plan_region(
    graph: &Graph,
    target: NodeId,
    request: Rect,
) -> Result<Vec<Step>, Error> {
    if !request.fits(graph.node(target)?.size) {
        return Err(Error::OutOfBounds);
    }

    let mut steps = Vec::new();
    let mut current = target;
    let mut region = request;

    loop {
        let node = graph.node(current)?;
        steps.push(Step {
            node: current,
            region,
        });
        match node.kind {
            NodeKind::Synthetic => break,
            NodeKind::Crop { input, rect } => {
                region = region.translate(rect.x, rect.y)?;
                current = input;
            }
            NodeKind::Resize { input } => {
                region = bilinear_footprint(region, graph.node(input)?.size, node.size)?;
                current = input;
            }
        }
    }
    steps.reverse();
    Ok(steps)
}

/// A one-dimensional interpolation location. Indices are clamped; weights
/// still follow the requested pixel-center convention at image boundaries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sample {
    pub lower: u32,
    pub upper: u32,
    pub upper_weight: f64,
}

pub(crate) fn sample_at(output: u32, input_len: u32, output_len: u32) -> Sample {
    debug_assert!(input_len > 0 && output_len > 0 && output < output_len);
    let mapped = (f64::from(output) + 0.5) * f64::from(input_len) / f64::from(output_len) - 0.5;
    let lower = mapped.floor();
    let upper = lower + 1.0;
    let max_index = f64::from(input_len - 1);
    Sample {
        lower: lower.clamp(0.0, max_index) as u32,
        upper: upper.clamp(0.0, max_index) as u32,
        upper_weight: mapped - lower,
    }
}

fn bilinear_footprint(output: Rect, input_size: Size, output_size: Size) -> Result<Rect, Error> {
    let first_x = sample_at(output.x, input_size.width, output_size.width);
    let last_x = sample_at(output.right() - 1, input_size.width, output_size.width);
    let first_y = sample_at(output.y, input_size.height, output_size.height);
    let last_y = sample_at(output.bottom() - 1, input_size.height, output_size.height);
    let left = first_x.lower;
    let top = first_y.lower;
    let right = last_x.upper;
    let bottom = last_y.upper;
    Rect::new(left, top, right - left + 1, bottom - top + 1)
}
