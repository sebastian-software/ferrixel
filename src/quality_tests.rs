use crate::quality::{Filter, footprint, taps};

#[test]
fn exact_area_average() {
    let samples = taps(4, 1, 0, Filter::Area).unwrap();
    assert_eq!(samples.len(), 4);
    for (x, tap) in samples.iter().enumerate() {
        assert_eq!(tap.index as usize, x);
        assert!((tap.weight - 0.25).abs() < 1.0e-12);
    }
}

#[test]
fn lanczos_weights_preserve_flat_color_at_edges() {
    for input in [1, 2, 7, 73, 512] {
        for output in [1, 3, 16, 91] {
            for y in 0..output {
                let samples = taps(input, output, y, Filter::Lanczos3).unwrap();
                let sum: f64 = samples.iter().map(|tap| tap.weight).sum();
                assert!((sum - 1.0).abs() < 1.0e-10);
                assert!(samples.iter().all(|tap| tap.index < input));
            }
        }
    }
}

#[test]
fn planner_footprint_covers_antialiasing_halo() {
    let (left, width) = footprint(4_000, 40, 10, 2, Filter::Lanczos3).unwrap();
    assert!(left < 1_000);
    assert!(left + width > 1_200);
    assert!(left + width <= 4_000);
    assert_eq!(footprint(7, 7, 0, 7, Filter::Area).unwrap(), (0, 7));
}

#[test]
fn geometry_fails_closed() {
    assert!(taps(0, 2, 0, Filter::Area).is_err());
    assert!(taps(10, 0, 0, Filter::Area).is_err());
    assert!(taps(10, 2, 2, Filter::Area).is_err());
    assert!(footprint(10, 2, 1, 2, Filter::Lanczos3).is_err());
}

#[test]
fn lanczos_zero_crossings_do_not_hide_interior_tap_indices() {
    // Input height 3 and output height 7 has a Lanczos zero at output
    // position 3. Position 4 then references input row zero again, so
    // endpoint-only footprint planning could underflow or underfetch.
    assert_eq!(footprint(3, 7, 3, 1, Filter::Lanczos3).unwrap(), (1, 1));
    assert_eq!(footprint(3, 7, 3, 2, Filter::Lanczos3).unwrap(), (0, 3));
    assert_eq!(footprint(3, 7, 2, 3, Filter::Lanczos3).unwrap(), (0, 3));

    for input in [1, 2, 3, 7, 23] {
        for output in [1, 3, 7, 11] {
            for start in 0..output {
                for count in 1..=output - start {
                    let (first, width) =
                        footprint(input, output, start, count, Filter::Lanczos3).unwrap();
                    for destination in start..start + count {
                        for tap in taps(input, output, destination, Filter::Lanczos3).unwrap() {
                            assert!(tap.index >= first);
                            assert!(tap.index < first + width);
                        }
                    }
                }
            }
        }
    }
}
