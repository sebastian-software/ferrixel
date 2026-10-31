//! Contract tests: metadata interpretation must never silently convert pixels.

use crate::color::{AlphaMode, ColorEncoding};
use crate::color_contract::{
    Cicp, CicpError, CicpFamily, ColorSignal, IccHeader, IccHeaderError, LinearSrgbReadiness,
    inspect_rendered_color, plan_to_linear_srgb,
};
use crate::decoder::{ChannelLayout, DecodeAccess, DecoderInfo, SampleType};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::Graph;

/// Header-only ICC fixture: deliberately NOT a valid color transform.
fn header_only_icc() -> Vec<u8> {
    let mut bytes = vec![0_u8; 132];
    bytes[0..4].copy_from_slice(&132_u32.to_be_bytes());
    bytes[8] = 4;
    bytes[16..20].copy_from_slice(b"RGB ");
    bytes[20..24].copy_from_slice(b"XYZ ");
    bytes[36..40].copy_from_slice(b"acsp");
    bytes
}

#[test]
fn known_cicp_families_are_distinguished_without_pixel_conversion() {
    let cases = [
        ([1, 13, 0, 1], CicpFamily::Srgb, LinearSrgbReadiness::TransferReferenceReady),
        ([12, 13, 0, 1], CicpFamily::DisplayP3, LinearSrgbReadiness::RequiresGamutConversion),
        ([9, 16, 0, 1], CicpFamily::Bt2100Pq, LinearSrgbReadiness::RequiresHdrManagement),
        ([9, 18, 9, 0], CicpFamily::Bt2100Hlg, LinearSrgbReadiness::RequiresHdrManagement),
        ([1, 13, 0, 0], CicpFamily::Other, LinearSrgbReadiness::UnknownSignal),
        ([1, 13, 1, 1], CicpFamily::Other, LinearSrgbReadiness::UnknownSignal),
        ([1, 1, 0, 1], CicpFamily::Other, LinearSrgbReadiness::UnknownSignal),
    ];
    for (raw, family, requirement) in cases {
        let signal = Cicp::try_from(raw).unwrap();
        assert_eq!(signal.family(), family, "{raw:?}");
        assert_eq!(inspect_rendered_color(None, Some(raw)), ColorSignal::Cicp(signal));
        assert_eq!(plan_to_linear_srgb(&ColorEncoding::Cicp(raw)), requirement);
    }
}

#[test]
fn malformed_range_is_not_assumed_to_be_srgb() {
    assert_eq!(Cicp::try_from([1, 13, 0, 2]), Err(CicpError::InvalidRangeFlag));
    assert_eq!(
        inspect_rendered_color(None, Some([1, 13, 0, 255])),
        ColorSignal::MalformedCicp(CicpError::InvalidRangeFlag)
    );
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::Cicp([1, 13, 0, 2])),
        LinearSrgbReadiness::MalformedColorEvidence
    );
}

#[test]
fn icc_header_inspection_never_qualifies_as_a_color_transform() {
    let bytes = header_only_icc();
    let header = IccHeader::inspect(&bytes).unwrap();
    assert_eq!(header.declared_size, 132);
    assert_eq!(header.version_major, 4);
    assert_eq!(header.data_color_space, *b"RGB ");
    assert_eq!(header.profile_connection_space, *b"XYZ ");
    assert_eq!(header.tag_count, 0);
    assert_eq!(inspect_rendered_color(Some(&bytes), None), ColorSignal::Icc(header));
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::Icc(bytes.into())),
        LinearSrgbReadiness::RequiresIccColorManagement
    );
}

#[test]
fn truncated_or_corrupt_icc_headers_fail_bounded_inspection() {
    let valid = header_only_icc();
    assert_eq!(IccHeader::inspect(&valid[..131]), Err(IccHeaderError::Truncated));

    let mut bad = valid.clone();
    bad[36..40].copy_from_slice(b"fake");
    assert_eq!(IccHeader::inspect(&bad), Err(IccHeaderError::InvalidSignature));

    bad = valid.clone();
    bad[0..4].copy_from_slice(&133_u32.to_be_bytes());
    assert_eq!(IccHeader::inspect(&bad), Err(IccHeaderError::InvalidDeclaredSize));

    bad = valid.clone();
    bad[0..4].copy_from_slice(&127_u32.to_be_bytes());
    assert_eq!(IccHeader::inspect(&bad), Err(IccHeaderError::InvalidDeclaredSize));

    bad = valid;
    bad[128..132].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(IccHeader::inspect(&bad), Err(IccHeaderError::InvalidTagTable));
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::Icc(bad.into())),
        LinearSrgbReadiness::MalformedColorEvidence
    );
}

#[test]
fn two_present_signals_require_explicit_precedence_even_when_both_look_srgb() {
    let icc = header_only_icc();
    let parsed = IccHeader::inspect(&icc).unwrap();
    let cicp = Cicp::try_from([1, 13, 0, 1]).unwrap();
    assert_eq!(
        inspect_rendered_color(Some(&icc), Some([1, 13, 0, 1])),
        ColorSignal::IccAndCicp { icc: parsed, cicp }
    );
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::IccAndCicp {
            icc: icc.into(),
            cicp: [1, 13, 0, 1],
        }),
        LinearSrgbReadiness::RequiresProfilePrecedence
    );
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::Unspecified),
        LinearSrgbReadiness::MissingColorEvidence
    );
}

#[test]
fn decoder_output_signal_survives_crop_but_blocks_unsupported_gamut() {
    let info = DecoderInfo {
        size: Size::new(12, 8).unwrap(),
        layout: ChannelLayout::Rgb,
        sample: SampleType::U8,
        access: DecodeAccess::CodecRegionRequest,
        alpha: AlphaMode::Opaque,
        rendered_icc: None,
        rendered_cicp: Some([12, 13, 0, 1]),
    };
    assert_eq!(
        info.rendered_color_signal(),
        ColorSignal::Cicp(Cicp::try_from([12, 13, 0, 1]).unwrap())
    );
    let mut graph = Graph::new();
    let source = graph
        .decoded_source(info.size, info.pixel_format(), info.semantics())
        .unwrap();
    let crop = graph.crop(source, Rect::new(2, 2, 5, 3).unwrap()).unwrap();
    assert_eq!(
        graph.node(source).unwrap().semantics,
        graph.node(crop).unwrap().semantics
    );
    assert_eq!(
        graph.resize_linear_srgb(crop, Size::new(3, 2).unwrap()),
        Err(Error::UnsupportedColorEncoding)
    );
    assert_eq!(
        graph.node(crop).unwrap().semantics.color.linear_srgb_readiness(),
        LinearSrgbReadiness::RequiresGamutConversion
    );
}

#[test]
fn invalid_icc_with_a_valid_cicp_is_not_promoted_to_srgb() {
    let invalid = vec![0_u8; 132];
    assert_eq!(
        inspect_rendered_color(Some(&invalid), Some([1, 13, 0, 1])),
        ColorSignal::MalformedIcc(IccHeaderError::InvalidSignature)
    );
    assert_eq!(
        plan_to_linear_srgb(&ColorEncoding::IccAndCicp {
            icc: invalid.into(),
            cicp: [1, 13, 0, 1],
        }),
        LinearSrgbReadiness::MalformedColorEvidence
    );
}
