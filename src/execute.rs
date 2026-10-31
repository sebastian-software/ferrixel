//! Experimental scalar execution with a non-retaining scanline sink.
//!
//! The caller owns its sink and may stream or collect the output. The engine
//! keeps only the currently requested source footprint and intermediate row
//! buffers; no complete image is constructed for a processing stage.

use crate::error::Error;
use crate::geometry::Rect;
use crate::graph::{Graph, NodeId, NodeKind};
use crate::memory::{ManagedBytes, MemoryBudget};
use crate::plan::{Step, plan_region, sample_at};
use crate::surface::PixelFormat;

#[derive(Debug)]
struct Tile {
    rect: Rect,
    pixels: ManagedBytes,
}

impl Tile {
    fn allocate(rect: Rect, budget: &MemoryBudget) -> Result<Self, Error> {
        Ok(Self {
            rect,
            pixels: budget.allocate(rect.byte_len()?)?,
        })
    }

    fn get(&self, x: u32, y: u32) -> u8 {
        debug_assert!(x >= self.rect.x && x < self.rect.right());
        debug_assert!(y >= self.rect.y && y < self.rect.bottom());
        let offset =
            (y - self.rect.y) as usize * self.rect.width as usize + (x - self.rect.x) as usize;
        self.pixels.as_slice()[offset]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Report {
    pub output_rows: u64,
    pub produced_pixels: u64,
    pub synthetic_pixels: u64,
    pub region_allocations: usize,
    pub total_allocated_bytes: usize,
    pub peak_managed_bytes: usize,
    pub full_frame_intermediates: u64,
}

/// The sink receives each row in output coordinates, in increasing Y order.
/// Its slice is borrowed only for the duration of the call. Error propagation
/// serves as backpressure/cancellation for the synchronous experiment.
pub(crate) fn execute(
    graph: &Graph,
    target: NodeId,
    output: Rect,
    budget: &MemoryBudget,
    mut sink: impl FnMut(u32, &[u8]) -> Result<(), Error>,
) -> Result<Report, Error> {
    if graph.node(target)?.format != PixelFormat::Gray8 {
        return Err(Error::PixelFormatMismatch);
    }
    if !output.fits(graph.node(target)?.size) {
        return Err(Error::OutOfBounds);
    }
    let baseline = budget.snapshot();
    let mut report = Report {
        output_rows: 0,
        produced_pixels: 0,
        synthetic_pixels: 0,
        region_allocations: 0,
        total_allocated_bytes: 0,
        peak_managed_bytes: 0,
        full_frame_intermediates: 0,
    };

    for y in output.y..output.bottom() {
        let requested_row = Rect::new(output.x, y, output.width, 1)?;
        let planned = plan_region(graph, target, requested_row)?;
        let result = execute_planned_region(graph, &planned, target, budget, &mut report)?;
        debug_assert_eq!(result.rect, requested_row);
        sink(y, result.pixels.as_slice())?;
        report.output_rows += 1;
        report.produced_pixels += u64::from(output.width);
        // `result` releases its managed allocation before the next row.
    }

    let snapshot = budget.snapshot();
    report.region_allocations = snapshot.allocations - baseline.allocations;
    report.total_allocated_bytes = snapshot.allocated_bytes - baseline.allocated_bytes;
    report.peak_managed_bytes = snapshot.peak;
    Ok(report)
}

/// Executes one planned region; independent of the sink and row scheduler.
fn execute_planned_region(
    graph: &Graph,
    steps: &[Step],
    target: NodeId,
    budget: &MemoryBudget,
    report: &mut Report,
) -> Result<Tile, Error> {
    let mut current: Option<Tile> = None;

    for step in steps {
        let node = graph.node(step.node)?;
        // Count nonterminal whole-image transform allocations, not input or sink data.
        if matches!(node.kind, NodeKind::Resize { .. })
            && step.node != target
            && step.region.x == 0
            && step.region.y == 0
            && step.region.width == node.size.width
            && step.region.height == node.size.height
        {
            report.full_frame_intermediates += 1;
        }
        let next = match node.kind {
            NodeKind::Planar10Source | NodeKind::DecoderSource => {
                    return Err(Error::PixelFormatMismatch);
                }
            NodeKind::Synthetic => {
                let mut tile = Tile::allocate(step.region, budget)?;
                for (row, line) in tile
                    .pixels
                    .as_mut_slice()
                    .chunks_exact_mut(step.region.width as usize)
                    .enumerate()
                {
                    let source_y =
                        step.region.y + u32::try_from(row).map_err(|_| Error::GeometryOverflow)?;
                    for (column, pixel) in line.iter_mut().enumerate() {
                        let source_x = step.region.x
                            + u32::try_from(column).map_err(|_| Error::GeometryOverflow)?;
                        *pixel = synthetic_pixel(source_x, source_y);
                    }
                }
                report.synthetic_pixels +=
                    u64::from(step.region.width) * u64::from(step.region.height);
                tile
            }
            NodeKind::Crop { .. } => {
                // Cropping is a rebase of a borrowed view; it copies no pixels.
                let mut tile = current.take().ok_or(Error::UnknownNode)?;
                debug_assert_eq!(tile.rect.width, step.region.width);
                debug_assert_eq!(tile.rect.height, step.region.height);
                tile.rect = step.region;
                tile
            }
            NodeKind::Resize { input } => {
                let source = current.take().ok_or(Error::UnknownNode)?;
                let input_size = graph.node(input)?.size;
                let mut tile = Tile::allocate(step.region, budget)?;
                let row_width = step.region.width as usize;
                for (row, line) in tile
                    .pixels
                    .as_mut_slice()
                    .chunks_exact_mut(row_width)
                    .enumerate()
                {
                    let output_y =
                        step.region.y + u32::try_from(row).map_err(|_| Error::GeometryOverflow)?;
                    let sy = sample_at(output_y, input_size.height, node.size.height);
                    for (column, pixel) in line.iter_mut().enumerate() {
                        let output_x = step.region.x
                            + u32::try_from(column).map_err(|_| Error::GeometryOverflow)?;
                        let sx = sample_at(output_x, input_size.width, node.size.width);
                        let top_left = f64::from(source.get(sx.lower, sy.lower));
                        let top_right = f64::from(source.get(sx.upper, sy.lower));
                        let bottom_left = f64::from(source.get(sx.lower, sy.upper));
                        let bottom_right = f64::from(source.get(sx.upper, sy.upper));
                        let horizontal_top = top_left + (top_right - top_left) * sx.upper_weight;
                        let horizontal_bottom =
                            bottom_left + (bottom_right - bottom_left) * sx.upper_weight;
                        *pixel = quantize_gray8(
                            horizontal_top + (horizontal_bottom - horizontal_top) * sy.upper_weight,
                        );
                    }
                }
                // `source` is released here before the next operation.
                tile
            }
        };
        current = Some(next);
    }

    current.ok_or(Error::UnknownNode)
}

pub(crate) fn synthetic_pixel(x: u32, y: u32) -> u8 {
    let a = x.wrapping_mul(53);
    let b = y.wrapping_mul(97);
    (a ^ b.rotate_left(5) ^ (x >> 3) ^ (y >> 2)).to_le_bytes()[0]
}

// Floating-point interpolation is explicitly clamped to the valid Gray8
// range. Rust intentionally has no TryFrom<f64> for u8; this narrow cast is
// therefore audited rather than turning off Clippy for all pixel operations.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn quantize_gray8(value: f64) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}
