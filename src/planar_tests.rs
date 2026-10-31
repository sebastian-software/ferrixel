//! Invariants for external, byte-strided, 10-bit planar source bindings.

use crate::error::Error;
use crate::execute::execute;
use crate::geometry::{Rect, Size};
use crate::graph::Graph;
use crate::memory::MemoryBudget;
use crate::plan::plan_region;
use crate::planar::execute_planar10;
use crate::surface::{
    ChromaSiting, ColorMetadata, ColorPrimaries, ColorRange, MatrixCoefficients, PixelFormat,
    PlaneId, PlaneView, TransferFunction, Yuv420P10, chroma_size,
};

fn size(width: u32, height: u32) -> Size {
    Size::new(width, height).unwrap()
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect::new(x, y, width, height).unwrap()
}

fn hdr_color() -> ColorMetadata {
    ColorMetadata {
        primaries: ColorPrimaries::Bt2020,
        transfer: TransferFunction::Pq,
        matrix: MatrixCoefficients::Bt2020NonConstantLuminance,
        range: ColorRange::Limited,
        chroma_siting: ChromaSiting::Centered,
    }
}

/// Synthetic unpacked samples with explicit padding between scanlines.
fn sample_value(x: u32, y: u32, seed: u16) -> u16 {
    u16::try_from((x * 71 + y * 129 + u32::from(seed)) % 1024).unwrap()
}

fn plane_bytes(size: Size, stride_bytes: usize, seed: u16) -> Vec<u8> {
    let mut data = vec![0xee; stride_bytes * size.height as usize];
    for y in 0..size.height {
        for x in 0..size.width {
            let pos = y as usize * stride_bytes + x as usize * 2;
            let sample = sample_value(x, y, seed).to_le_bytes();
            data[pos..pos + 2].copy_from_slice(&sample);
        }
    }
    data
}

#[test]
fn borrowed_plane_rejects_invalid_stride_or_storage() {
    let data = vec![0; 16];
    assert!(matches!(
        PlaneView::new(&data, size(5, 1), 9),
        Err(Error::InvalidPlaneStride)
    ));
    assert!(matches!(
        PlaneView::new(&data, size(5, 2), 10),
        Err(Error::InsufficientPlaneStorage)
    ));
    // The last row need not contain the trailing padding bytes.
    let exact = vec![0; 11 * 2 + 10];
    assert!(PlaneView::new(&exact, size(5, 3), 11).is_ok());
    assert_eq!(chroma_size(size(7, 5)).unwrap(), size(4, 3));
}

#[test]
fn plane_crop_borrows_unaligned_source_with_original_stride() {
    let dimensions = size(7, 5);
    let stride = 17; // odd-byte stride; the implementation must not cast to &[u16].
    let bytes = plane_bytes(dimensions, stride, 100);
    let plane = PlaneView::new(&bytes, dimensions, stride).unwrap();
    let crop = plane.crop(rect(2, 1, 3, 3)).unwrap();
    assert_eq!(crop.size(), size(3, 3));
    assert_eq!(crop.stride_bytes(), stride);
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(crop.sample(x, y).unwrap(), sample_value(x + 2, y + 1, 100));
        }
    }
    let row = crop.row(0, 0, 3).unwrap();
    assert_eq!(row.as_ptr(), bytes[17 + 4..].as_ptr());
    assert_eq!(row.len(), 6); // excludes padding and neighboring samples
    assert_eq!(row[0], bytes[21]);
    assert!(matches!(
        plane.crop(rect(6, 4, 2, 1)),
        Err(Error::OutOfBounds)
    ));
}

#[test]
fn planar_source_requires_exact_chroma_geometry() {
    let luma = size(7, 5);
    let chroma = size(4, 3);
    let ybytes = plane_bytes(luma, 15, 1);
    let cbytes = plane_bytes(chroma, 9, 2);
    let y = PlaneView::new(&ybytes, luma, 15).unwrap();
    let c = PlaneView::new(&cbytes, chroma, 9).unwrap();
    assert!(Yuv420P10::new(luma, hdr_color(), y, c, c).is_ok());
    assert!(matches!(
        Yuv420P10::new(luma, hdr_color(), y, y, c),
        Err(Error::InvalidPlaneDimensions)
    ));
}

#[test]
fn planar_crop_forwards_padded_10bit_rows_without_copies() {
    let dimensions = size(12, 10);
    let chroma = chroma_size(dimensions).unwrap();
    let ybytes = plane_bytes(dimensions, 29, 1);
    let blue_storage = plane_bytes(chroma, 15, 2);
    let red_storage = plane_bytes(chroma, 17, 3);
    let surface = Yuv420P10::new(
        dimensions,
        hdr_color(),
        PlaneView::new(&ybytes, dimensions, 29).unwrap(),
        PlaneView::new(&blue_storage, chroma, 15).unwrap(),
        PlaneView::new(&red_storage, chroma, 17).unwrap(),
    )
    .unwrap();

    let mut graph = Graph::new();
    let source = graph.planar_10(dimensions, hdr_color());
    let crop = graph.crop(source, rect(2, 2, 7, 5)).unwrap();
    let output = rect(2, 2, 5, 3);
    let upstream = plan_region(&graph, crop, output).unwrap();
    assert_eq!(upstream[0].region, rect(4, 4, 5, 3));
    assert_eq!(
        graph.node(crop).unwrap().format,
        PixelFormat::Yuv420P10(hdr_color())
    );

    let mut observed = Vec::new();
    let report = execute_planar10(&graph, crop, source, &surface, output, |p, y, row| {
        let (source_x, source_y, width) = match p {
            PlaneId::Y => (4, y + 2, 5),
            PlaneId::Cb | PlaneId::Cr => (2, y + 1, 3),
        };
        let source_row = surface.plane(p).row(source_y, source_x, width).unwrap();
        assert_eq!(row, source_row);
        assert_eq!(row.as_ptr(), source_row.as_ptr());
        observed.push((p, y, row.to_vec()));
        Ok(())
    })
    .unwrap();

    assert_eq!(report.forwarded_rows, 7); // Y: 3, Cb: 2, Cr: 2
    assert_eq!(report.forwarded_bytes, 54); // Y: 30, chroma: 12 each
    assert_eq!(report.managed_pixel_allocations, 0);
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(observed[0].1, 2); // output-space luma coordinates
    assert_eq!(observed[3].0, PlaneId::Cb);
    assert_eq!(observed[3].1, 1); // output-space chroma coordinates
    assert_eq!(observed[5].0, PlaneId::Cr);
}

#[test]
fn odd_luma_dimensions_map_to_ceil_chroma_dimensions() {
    let dimensions = size(7, 5);
    let chroma = chroma_size(dimensions).unwrap();
    let ybytes = plane_bytes(dimensions, 15, 1);
    let blue_storage = plane_bytes(chroma, 9, 2);
    let red_storage = plane_bytes(chroma, 9, 3);
    let surface = Yuv420P10::new(
        dimensions,
        hdr_color(),
        PlaneView::new(&ybytes, dimensions, 15).unwrap(),
        PlaneView::new(&blue_storage, chroma, 9).unwrap(),
        PlaneView::new(&red_storage, chroma, 9).unwrap(),
    )
    .unwrap();
    let mut graph = Graph::new();
    let source = graph.planar_10(dimensions, hdr_color());
    let crop = graph.crop(source, rect(0, 0, 7, 5)).unwrap();
    let report = execute_planar10(
        &graph,
        crop,
        source,
        &surface,
        rect(0, 0, 7, 5),
        |p, _, row| {
            let wanted = match p {
                PlaneId::Y => 14,
                PlaneId::Cb | PlaneId::Cr => 8,
            };
            assert_eq!(row.len(), wanted);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(report.forwarded_rows, 11);
    assert_eq!(report.forwarded_bytes, 5 * 14 + 2 * 3 * 8);
}

#[test]
fn unaligned_phase_and_unvalidated_planar_resize_are_rejected() {
    let mut graph = Graph::new();
    let source = graph.planar_10(size(8, 8), hdr_color());
    assert_eq!(
        graph.crop(source, rect(1, 0, 4, 4)),
        Err(Error::ChromaPhaseRequiresResampling)
    );
    assert_eq!(
        graph.crop(source, rect(0, 1, 4, 4)),
        Err(Error::ChromaPhaseRequiresResampling)
    );
    assert_eq!(
        graph.resize(source, size(4, 4)),
        Err(Error::UnsupportedPixelOperation)
    );
    let y = vec![0; 8 * 8 * 2];
    let c = vec![0; 4 * 4 * 2];
    let surface = Yuv420P10::new(
        size(8, 8),
        hdr_color(),
        PlaneView::new(&y, size(8, 8), 16).unwrap(),
        PlaneView::new(&c, size(4, 4), 8).unwrap(),
        PlaneView::new(&c, size(4, 4), 8).unwrap(),
    )
    .unwrap();
    let bad = execute_planar10(
        &graph,
        source,
        source,
        &surface,
        rect(1, 0, 4, 4),
        |_, _, _| Ok(()),
    );
    assert_eq!(bad, Err(Error::ChromaPhaseRequiresResampling));
    assert_eq!(
        execute(
            &graph,
            source,
            rect(0, 0, 4, 4),
            &MemoryBudget::new(1_024),
            |_, _| Ok(())
        ),
        Err(Error::PixelFormatMismatch)
    );
}

#[test]
fn mismatched_external_binding_is_rejected_before_forwarding() {
    let dims = size(4, 4);
    let ybytes = vec![0; 32];
    let cbytes = vec![0; 8];
    let y = PlaneView::new(&ybytes, dims, 8).unwrap();
    let c = PlaneView::new(&cbytes, size(2, 2), 4).unwrap();
    let surface = Yuv420P10::new(dims, hdr_color(), y, c, c).unwrap();
    let mut graph = Graph::new();
    let source = graph.planar_10(dims, hdr_color());
    let another = graph.planar_10(dims, hdr_color());
    let wrong_color = ColorMetadata {
        primaries: ColorPrimaries::Bt709,
        transfer: TransferFunction::Srgb,
        matrix: MatrixCoefficients::Bt709,
        range: ColorRange::Full,
        chroma_siting: ChromaSiting::Cosited,
    };
    let other_color = graph.planar_10(dims, wrong_color);
    let roi = rect(0, 0, 2, 2);
    assert_eq!(
        execute_planar10(&graph, source, another, &surface, roi, |_, _, _| Ok(())),
        Err(Error::SourceBindingMismatch)
    );
    assert_eq!(
        execute_planar10(
            &graph,
            other_color,
            other_color,
            &surface,
            roi,
            |_, _, _| Ok(())
        ),
        Err(Error::SourceBindingMismatch)
    );
}

#[test]
fn cancellation_stops_forwarding_and_does_not_mutate_external_buffers() {
    let dims = size(4, 4);
    let ybytes = plane_bytes(dims, 9, 7);
    let cbytes = plane_bytes(size(2, 2), 5, 11);
    let y_before = ybytes.clone();
    let c_before = cbytes.clone();
    let surface = Yuv420P10::new(
        dims,
        hdr_color(),
        PlaneView::new(&ybytes, dims, 9).unwrap(),
        PlaneView::new(&cbytes, size(2, 2), 5).unwrap(),
        PlaneView::new(&cbytes, size(2, 2), 5).unwrap(),
    )
    .unwrap();
    let mut graph = Graph::new();
    let source = graph.planar_10(dims, hdr_color());
    let mut seen = 0;
    let result = execute_planar10(
        &graph,
        source,
        source,
        &surface,
        rect(0, 0, 4, 4),
        |_, _, _| {
            seen += 1;
            Err(Error::Cancelled)
        },
    );
    assert_eq!(result, Err(Error::Cancelled));
    assert_eq!(seen, 1);
    assert_eq!(ybytes, y_before);
    assert_eq!(cbytes, c_before);
}

#[test]
fn synthetic_bit_depth_roundtrips_through_borrowed_bytes() {
    let dimensions = size(9, 3);
    let stride = 21;
    let bytes = plane_bytes(dimensions, stride, 900);
    let view = PlaneView::new(&bytes, dimensions, stride).unwrap();
    for y in 0..dimensions.height {
        for x in 0..dimensions.width {
            let sample = view.sample(x, y).unwrap();
            assert!(sample <= 1023);
            assert_eq!(sample, sample_value(x, y, 900));
        }
    }
}

#[test]
fn hlg_metadata_can_be_preserved_without_implicit_conversion() {
    let hlg = ColorMetadata {
        transfer: TransferFunction::Hlg,
        ..hdr_color()
    };
    let mut graph = Graph::new();
    let source = graph.planar_10(size(2, 2), hlg);
    let crop = graph.crop(source, rect(0, 0, 2, 2)).unwrap();
    assert_eq!(
        graph.node(crop).unwrap().format,
        PixelFormat::Yuv420P10(hlg)
    );
}
