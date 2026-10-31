//! Inspect emitted color evidence without converting pixels.
//!
//! Decoder adapters must describe *rendered output samples*, not simply copy
//! metadata from the encoded file. Missing and dual-signalled metadata require
//! deliberate policy choices; inspection never assumes sRGB or ICC precedence.

use crate::color::ColorEncoding;

/// H.273 coding-independent color points. Unsupported values remain intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cicp {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CicpError {
    InvalidRangeFlag,
}

impl TryFrom<[u8; 4]> for Cicp {
    type Error = CicpError;

    fn try_from(raw: [u8; 4]) -> Result<Self, Self::Error> {
        let full_range = match raw[3] {
            0 => false,
            1 => true,
            _ => return Err(CicpError::InvalidRangeFlag),
        };
        Ok(Self {
            primaries: raw[0],
            transfer: raw[1],
            matrix: raw[2],
            full_range,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CicpFamily {
    Srgb,
    DisplayP3,
    Bt2100Pq,
    Bt2100Hlg,
    Other,
}

impl Cicp {
    pub(crate) fn family(self) -> CicpFamily {
        match (self.primaries, self.transfer, self.matrix, self.full_range) {
            (1, 13, 0, true) => CicpFamily::Srgb,
            (12, 13, 0, true) => CicpFamily::DisplayP3,
            (9, 16, 0 | 9, _) => CicpFamily::Bt2100Pq,
            (9, 18, 0 | 9, _) => CicpFamily::Bt2100Hlg,
            _ => CicpFamily::Other,
        }
    }
}

/// Only a checked ICC header and tag-table boundary, NOT a validated ICC
/// transform. A well-formed header cannot establish gamut or transfer details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IccHeader {
    pub declared_size: u32,
    pub version_major: u8,
    pub data_color_space: [u8; 4],
    pub profile_connection_space: [u8; 4],
    pub tag_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IccHeaderError {
    Truncated,
    InvalidSignature,
    InvalidDeclaredSize,
    InvalidTagTable,
}

impl IccHeader {
    pub(crate) fn inspect(bytes: &[u8]) -> Result<Self, IccHeaderError> {
        // An ICC header is 128 bytes; a 4-byte tag count follows.
        if bytes.len() < 132 {
            return Err(IccHeaderError::Truncated);
        }
        if &bytes[36..40] != b"acsp" {
            return Err(IccHeaderError::InvalidSignature);
        }
        let declared_size = u32::from_be_bytes(
            bytes[0..4]
                .try_into()
                .map_err(|_| IccHeaderError::Truncated)?,
        );
        let declared = usize::try_from(declared_size)
            .map_err(|_| IccHeaderError::InvalidDeclaredSize)?;
        if declared < 132 || declared > bytes.len() {
            return Err(IccHeaderError::InvalidDeclaredSize);
        }
        let tag_count = u32::from_be_bytes(
            bytes[128..132]
                .try_into()
                .map_err(|_| IccHeaderError::Truncated)?,
        );
        let table_end = usize::try_from(tag_count)
            .ok()
            .and_then(|count| count.checked_mul(12))
            .and_then(|size| size.checked_add(132))
            .ok_or(IccHeaderError::InvalidTagTable)?;
        if table_end > declared {
            return Err(IccHeaderError::InvalidTagTable);
        }
        Ok(Self {
            declared_size,
            version_major: bytes[8],
            data_color_space: bytes[16..20]
                .try_into()
                .map_err(|_| IccHeaderError::Truncated)?,
            profile_connection_space: bytes[20..24]
                .try_into()
                .map_err(|_| IccHeaderError::Truncated)?,
            tag_count,
        })
    }
}

/// Color description, independent of pixel storage and rendering backend.
/// Dual-signal profile precedence is intentionally NOT decided by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorSignal {
    Unspecified,
    Cicp(Cicp),
    Icc(IccHeader),
    IccAndCicp { icc: IccHeader, cicp: Cicp },
    MalformedCicp(CicpError),
    MalformedIcc(IccHeaderError),
}

/// Requirements for eventual conversion into linear sRGB. This is a
/// capability decision, not a promise that the conversion is implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinearSrgbReadiness {
    TransferReferenceReady,
    RequiresGamutConversion,
    RequiresHdrManagement,
    RequiresIccColorManagement,
    RequiresProfilePrecedence,
    MissingColorEvidence,
    UnknownSignal,
    MalformedColorEvidence,
}

/// No heap allocations and no color transforms. Only inspect emitted signals.
pub(crate) fn inspect_rendered_color(icc: Option<&[u8]>, cicp: Option<[u8; 4]>) -> ColorSignal {
    match (icc, cicp) {
        (None, None) => ColorSignal::Unspecified,
        (None, Some(raw)) => match Cicp::try_from(raw) {
            Ok(signal) => ColorSignal::Cicp(signal),
            Err(error) => ColorSignal::MalformedCicp(error),
        },
        (Some(bytes), None) => match IccHeader::inspect(bytes) {
            Ok(header) => ColorSignal::Icc(header),
            Err(error) => ColorSignal::MalformedIcc(error),
        },
        (Some(bytes), Some(raw)) => {
            let header = match IccHeader::inspect(bytes) {
                Ok(header) => header,
                Err(error) => return ColorSignal::MalformedIcc(error),
            };
            match Cicp::try_from(raw) {
                Ok(signal) => ColorSignal::IccAndCicp {
                    icc: header,
                    cicp: signal,
                },
                Err(error) => ColorSignal::MalformedCicp(error),
            }
        }
    }
}

pub(crate) fn plan_to_linear_srgb(color: &ColorEncoding) -> LinearSrgbReadiness {
    let inspected = match color {
        ColorEncoding::Unspecified => inspect_rendered_color(None, None),
        ColorEncoding::Icc(icc) => inspect_rendered_color(Some(icc.as_ref()), None),
        ColorEncoding::Cicp(raw) => inspect_rendered_color(None, Some(*raw)),
        ColorEncoding::IccAndCicp { icc, cicp } => {
            inspect_rendered_color(Some(icc.as_ref()), Some(*cicp))
        }
    };
    match inspected {
        ColorSignal::Unspecified => LinearSrgbReadiness::MissingColorEvidence,
        ColorSignal::Cicp(cicp) => match cicp.family() {
            CicpFamily::Srgb => LinearSrgbReadiness::TransferReferenceReady,
            CicpFamily::DisplayP3 => LinearSrgbReadiness::RequiresGamutConversion,
            CicpFamily::Bt2100Pq | CicpFamily::Bt2100Hlg => {
                LinearSrgbReadiness::RequiresHdrManagement
            }
            CicpFamily::Other => LinearSrgbReadiness::UnknownSignal,
        },
        ColorSignal::Icc(_) => LinearSrgbReadiness::RequiresIccColorManagement,
        ColorSignal::IccAndCicp { .. } => LinearSrgbReadiness::RequiresProfilePrecedence,
        ColorSignal::MalformedCicp(_) | ColorSignal::MalformedIcc(_) => {
            LinearSrgbReadiness::MalformedColorEvidence
        }
    }
}
