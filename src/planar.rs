//! Experimental execution of borrowed, 10-bit planar crops.
//!
//! This deliberately supports *only* coordinate-only crop chains. It proves
//! that the existing graph and backward region planner can work with an
//! externally owned, byte-strided YUV surface without creating pixel buffers.
//! Resizing is not enabled until chroma siting and color quality are specified.

use crate::error::Error;
use crate::geometry::Rect;
use crate::graph::{Graph, NodeId, NodeKind};
use crate::plan::plan_region;
use crate::surface::{PixelFormat, PlaneId, Yuv420P10, chroma_rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlanarReport {
    /// Number of rows forwarded across all three planes.
    pub forwarded_rows: u64,
    /// Includes two bytes per 10-bit sample, excludes stride padding.
    pub forwarded_bytes: u64,
    /// Pixel allocations, not the graph or execution-plan metadata.
    pub managed_pixel_allocations: u64,
    pub full_frame_intermediates: u64,
}

/// Bind a borrowed surface to the semantic source node at execution time.
///
/// The surface must outlive the call. The row callback borrows its input; it
/// may copy into a caller-owned sink but the executor never materializes pixels.
/// Rows arrive in Y, Cb, Cr plane order, ascending per-plane Y coordinates.
/// A partial output request must have an even luma origin for correct 4:2:0
/// phase preservation. Output width/height may be odd.
pub(crate) fn execute_planar10(
    graph: &Graph,
    target: NodeId,
    bound_source: NodeId,
    surface: &Yuv420P10<'_>,
    output: Rect,
    mut sink: impl FnMut(PlaneId, u32, &[u8]) -> Result<(), Error>,
) -> Result<PlanarReport, Error> {
    let node = graph.node(target)?;
    if !matches!(node.format, PixelFormat::Yuv420P10(_)) {
        return Err(Error::PixelFormatMismatch);
    }
    if !output.fits(node.size) {
        return Err(Error::OutOfBounds);
    }
    let chroma_output = chroma_rect(output)?;
    let planned = plan_region(graph, target, output)?;
    let source_step = planned.first().ok_or(Error::UnknownNode)?;
    let source_node = graph.node(source_step.node)?;
    if source_step.node != bound_source
        || source_node.kind != NodeKind::Planar10Source
        || source_node.size != surface.size()
        || source_node.format != PixelFormat::Yuv420P10(surface.color())
    {
        return Err(Error::SourceBindingMismatch);
    }

    // The graph may grow new nodes in future, but this executor is restricted
    // to crop-only chains: forwarding a resize without computing it would be
    // incorrect even if its final footprint looked compatible.
    if planned.iter().skip(1).any(|step| {
        !matches!(graph.node(step.node), Ok(n) if matches!(n.kind, NodeKind::Crop { .. }))
    }) {
        return Err(Error::UnsupportedPixelOperation);
    }

    let source_luma = source_step.region;
    let source_chroma = chroma_rect(source_luma)?;
    let mut report = PlanarReport {
        forwarded_rows: 0,
        forwarded_bytes: 0,
        managed_pixel_allocations: 0,
        full_frame_intermediates: 0,
    };

    for plane in PlaneId::ALL {
        let (requested, source) = match plane {
            PlaneId::Y => (output, source_luma),
            PlaneId::Cb | PlaneId::Cr => (chroma_output, source_chroma),
        };
        let view = surface.plane(plane);
        for row in 0..requested.height {
            let source_y = source.y + row;
            let output_y = requested.y + row;
            let samples = view.row(source_y, source.x, source.width)?;
            sink(plane, output_y, samples)?;
            report.forwarded_rows += 1;
            report.forwarded_bytes +=
                u64::try_from(samples.len()).map_err(|_| Error::GeometryOverflow)?;
        }
    }

    Ok(report)
}
