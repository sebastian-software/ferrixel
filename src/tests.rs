//! Differential and invariance tests for the codec-free vertical slice.

use crate::error::Error;
use crate::execute::{Report, execute, synthetic_pixel};
use crate::geometry::{Rect, Size};
use crate::graph::Graph;
use crate::memory::MemoryBudget;
use crate::plan::plan_region;

fn size(w: u32, h: u32) -> Size {
    Size::new(w, h).unwrap()
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}

fn collect(
    graph: &Graph,
    id: crate::graph::NodeId,
    region: Rect,
    budget: &MemoryBudget,
) -> (Vec<u8>, Report) {
    let mut bytes = Vec::new();
    let report = execute(graph, id, region, budget, |_, row| {
        bytes.extend_from_slice(row);
        Ok(())
    })
    .unwrap();
    (bytes, report)
}

fn reference_resize(input: &[u8], input_size: Size, output_size: Size) -> Vec<u8> {
    // Deliberately independent of `sample_at`, the planner, and the region
    // executor. This eager implementation acts as a correctness oracle.
    let mut output = vec![0; (output_size.width * output_size.height) as usize];
    for y in 0..output_size.height {
        let sy = (f64::from(y) + 0.5) * f64::from(input_size.height)
            / f64::from(output_size.height)
            - 0.5;
        let y0 = sy.floor();
        let fy = sy - y0;
        for x in 0..output_size.width {
            let sx = (f64::from(x) + 0.5) * f64::from(input_size.width)
                / f64::from(output_size.width)
                - 0.5;
            let x0 = sx.floor();
            let fx = sx - x0;
            let at = |px: f64, py: f64| {
                let col = px.clamp(0.0, f64::from(input_size.width - 1)) as usize;
                let row = py.clamp(0.0, f64::from(input_size.height - 1)) as usize;
                f64::from(input[row * input_size.width as usize + col])
            };
            let a = at(x0, y0);
            let b = at(x0 + 1.0, y0);
            let c = at(x0, y0 + 1.0);
            let d = at(x0 + 1.0, y0 + 1.0);
            output[(y * output_size.width + x) as usize] =
                ((a + (b - a) * fx) * (1.0 - fy) + (c + (d - c) * fx) * fy).round() as u8;
        }
    }
    output
}

#[test]
fn synthetic_crop_is_a_copy_free_coordinate_rebase() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(60, 40));
    let crop = graph.crop(source, rect(17, 12, 20, 16)).unwrap();
    let budget = MemoryBudget::new(512);
    let roi = rect(4, 5, 8, 7);
    let (bytes, report) = collect(&graph, crop, roi, &budget);
    for y in 0..roi.height {
        for x in 0..roi.width {
            let got = bytes[(y * roi.width + x) as usize];
            assert_eq!(got, synthetic_pixel(17 + roi.x + x, 12 + roi.y + y));
        }
    }
    assert_eq!(report.synthetic_pixels, 8 * 7);
    assert_eq!(report.region_allocations, 7); // one source buffer/row, no crop allocation
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn crop_then_resize_matches_independent_eager_reference() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(47, 31));
    let crop_area = rect(7, 4, 19, 13);
    let crop = graph.crop(source, crop_area).unwrap();
    let resized = graph.resize(crop, size(9, 8)).unwrap();
    let budget = MemoryBudget::new(512);
    let roi = rect(2, 1, 5, 6);
    let (actual, report) = collect(&graph, resized, roi, &budget);

    let mut eager_crop = Vec::new();
    for y in crop_area.y..crop_area.bottom() {
        for x in crop_area.x..crop_area.right() {
            eager_crop.push(synthetic_pixel(x, y));
        }
    }
    let eager = reference_resize(&eager_crop, size(19, 13), size(9, 8));
    let mut expected = Vec::new();
    for y in roi.y..roi.bottom() {
        let offset = (y * 9 + roi.x) as usize;
        expected.extend_from_slice(&eager[offset..offset + roi.width as usize]);
    }
    assert_eq!(actual, expected);
    assert!(report.synthetic_pixels < 47 * 31);
    assert!(report.peak_managed_bytes < 512);
    assert_eq!(report.full_frame_intermediates, 0);
}

#[test]
fn tall_source_uses_only_the_requested_rows() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(128, 100_000_000));
    let cropped = graph.crop(source, rect(20, 99_999_600, 40, 200)).unwrap();
    let resized = graph.resize(cropped, size(20, 50)).unwrap();
    let budget = MemoryBudget::new(256);
    let (_, report) = collect(&graph, resized, rect(3, 4, 7, 5), &budget);
    assert!(report.synthetic_pixels <= 5 * 2 * 40);
    assert!(report.peak_managed_bytes <= 87);
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn aggressive_downscale_does_not_materialize_gaps_between_input_rows() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(2_048, 10_000_000));
    let resized = graph.resize(source, size(8, 8)).unwrap();
    let budget = MemoryBudget::new(4_200);
    let (_, report) = collect(&graph, resized, rect(0, 0, 8, 8), &budget);
    assert!(report.synthetic_pixels <= 8 * 2 * 2_048);
    assert!(report.peak_managed_bytes <= 2 * 2_048 + 8);
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(budget.snapshot().live, 0);

    // A whole-output bounding rectangle would accidentally include nearly all
    // 10 million source scanlines; planning one row only requests two.
    let first = plan_region(&graph, resized, rect(0, 0, 8, 1)).unwrap();
    assert_eq!(first[0].region.height, 2);
}

#[test]
fn chaining_resizes_and_downstream_crop_preserves_semantics() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(25, 23));
    let first = graph.resize(source, size(13, 17)).unwrap();
    let second = graph.resize(first, size(29, 11)).unwrap();
    let cropped = graph.crop(second, rect(2, 3, 18, 6)).unwrap();
    let roi = rect(3, 1, 7, 4);
    let (actual, _) = collect(&graph, cropped, roi, &MemoryBudget::new(4_096));

    let mut eager_source = Vec::new();
    for y in 0..23 {
        for x in 0..25 {
            eager_source.push(synthetic_pixel(x, y));
        }
    }
    let first_eager = reference_resize(&eager_source, size(25, 23), size(13, 17));
    let second_eager = reference_resize(&first_eager, size(13, 17), size(29, 11));
    let mut expected = Vec::new();
    for y in roi.y..roi.bottom() {
        let global_y = 3 + y;
        let global_x = 2 + roi.x;
        let offset = (global_y * 29 + global_x) as usize;
        expected.extend_from_slice(&second_eager[offset..offset + roi.width as usize]);
    }
    assert_eq!(actual, expected);
}

#[test]
fn insufficient_budget_releases_all_owned_buffers() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(100, 100));
    let resized = graph.resize(source, size(90, 90)).unwrap();
    let budget = MemoryBudget::new(250);
    let result = execute(&graph, resized, rect(0, 0, 90, 1), &budget, |_, _| Ok(()));
    assert!(matches!(result, Err(Error::BudgetExceeded { .. })));
    assert_eq!(budget.snapshot().live, 0);
    assert!(budget.snapshot().allocations > 0);
}

#[test]
fn cancellation_from_sink_releases_buffers() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(64, 800));
    let resized = graph.resize(source, size(32, 400)).unwrap();
    let budget = MemoryBudget::new(256);
    let mut rows = 0;
    let result = execute(&graph, resized, rect(0, 0, 32, 400), &budget, |_, _| {
        rows += 1;
        if rows == 3 {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    });
    assert_eq!(result, Err(Error::Cancelled));
    assert_eq!(rows, 3);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn bounds_are_checked_before_execution_or_allocation() {
    let mut graph = Graph::new();
    let source = graph.synthetic(size(10, 10));
    assert_eq!(graph.crop(source, rect(7, 7, 4, 4)), Err(Error::OutOfBounds));
    assert_eq!(Size::new(0, 1), Err(Error::InvalidDimensions));
    assert_eq!(Rect::new(u32::MAX, 0, 2, 1), Err(Error::GeometryOverflow));
    let budget = MemoryBudget::new(100);
    let failed = execute(&graph, source, rect(8, 8, 3, 1), &budget, |_, _| Ok(()));
    assert_eq!(failed, Err(Error::OutOfBounds));
    assert_eq!(budget.snapshot().allocations, 0);
}

#[test]
fn shared_budget_prevents_concurrent_overcommit() {
    use std::sync::{Arc, Barrier};
    let budget = MemoryBudget::new(64);
    let held = budget.allocate(40).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let other_budget = budget.clone();
    let other_barrier = Arc::clone(&barrier);
    let thread = std::thread::spawn(move || {
        other_barrier.wait();
        other_budget.allocate(40).err()
    });
    barrier.wait();
    assert!(matches!(thread.join().unwrap(), Some(Error::BudgetExceeded { .. })));
    assert_eq!(budget.snapshot().live, 40);
    drop(held);
    assert_eq!(budget.snapshot().live, 0);
}
