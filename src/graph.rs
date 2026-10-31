//! A small internal semantic graph used to validate planning contracts.
//!
//! Every new node may only refer to an already-existing node. This keeps the
//! initial graph immutable after construction and makes cycles impossible.

use crate::color::{AlphaMode, ImageSemantics};
use crate::color_contract::LinearSrgbReadiness;
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::surface::{ChannelLayout, ColorMetadata, PixelFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeId(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Deterministic, coordinate-addressable synthetic source.
    Synthetic,
    Planar10Source,
    DecoderSource,
    Crop {
        input: NodeId,
        rect: Rect,
    },
    /// A reference scalar resize with an explicit sample-domain policy.
    Resize {
        input: NodeId,
        domain: ResizeDomain,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeDomain {
    EncodedSamples,
    /// Reference bilinear filtering in linear sRGB, with straight-alpha
    /// inputs premultiplied *after* transfer decoding and before filtering.
    LinearSrgb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub size: Size,
    pub format: PixelFormat,
    pub semantics: ImageSemantics,
}

#[derive(Default)]
pub(crate) struct Graph {
    nodes: Vec<Node>,
}

impl Graph {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn synthetic(&mut self, size: Size) -> NodeId {
        self.push(Node {
            kind: NodeKind::Synthetic,
            size,
            format: PixelFormat::Gray8,
            semantics: ImageSemantics::unspecified_opaque(),
        })
    }

    /// Register a semantic 10-bit source. Its external plane storage is bound
    /// separately when the executor runs; the graph never owns input buffers.
    pub(crate) fn planar_10(&mut self, size: Size, color: ColorMetadata) -> NodeId {
        self.push(Node {
            kind: NodeKind::Planar10Source,
            size,
            format: PixelFormat::Yuv420P10(color),
            semantics: ImageSemantics::unspecified_opaque(),
        })
    }

    /// Bind an encoded source at execution time; the graph owns no pixel data.
    pub(crate) fn decoded_source(
        &mut self,
        size: Size,
        format: PixelFormat,
        semantics: ImageSemantics,
    ) -> Result<NodeId, Error> {
        if !matches!(
            format,
            PixelFormat::InterleavedU8(_) | PixelFormat::InterleavedF32(_)
        ) {
            return Err(Error::PixelFormatMismatch);
        }
        Ok(self.push(Node {
            kind: NodeKind::DecoderSource,
            size,
            format,
            semantics,
        }))
    }

    pub(crate) fn crop(&mut self, input: NodeId, rect: Rect) -> Result<NodeId, Error> {
        let parent = self.node(input)?;
        if !rect.fits(parent.size) {
            return Err(Error::OutOfBounds);
        }
        if matches!(parent.format, PixelFormat::Yuv420P10(_))
            && (!rect.x.is_multiple_of(2) || !rect.y.is_multiple_of(2))
        {
            return Err(Error::ChromaPhaseRequiresResampling);
        }
        Ok(self.push(Node {
            kind: NodeKind::Crop { input, rect },
            size: Size::new(rect.width, rect.height)?,
            format: parent.format,
            semantics: parent.semantics.clone(),
        }))
    }

    pub(crate) fn resize(&mut self, input: NodeId, size: Size) -> Result<NodeId, Error> {
        let parent = self.node(input)?;
        if parent.format != PixelFormat::Gray8 {
            // A dedicated chroma-aware resize and color-quality policy is
            // needed before a planar transform can be exposed safely.
            return Err(Error::UnsupportedPixelOperation);
        }
        Ok(self.push(Node {
            kind: NodeKind::Resize {
                input,
                domain: ResizeDomain::EncodedSamples,
            },
            size,
            format: parent.format,
            semantics: parent.semantics.clone(),
        }))
    }

    /// A bilinear sample-domain reference, not linear-light color management.
    pub(crate) fn resize_encoded_samples(
        &mut self,
        input: NodeId,
        size: Size,
    ) -> Result<NodeId, Error> {
        let parent = self.node(input)?;
        if !matches!(
            parent.format,
            PixelFormat::InterleavedU8(ChannelLayout::Gray | ChannelLayout::Rgb)
                | PixelFormat::InterleavedF32(ChannelLayout::Gray | ChannelLayout::Rgb)
        ) || parent.semantics.alpha != AlphaMode::Opaque
        {
            return Err(Error::UnsupportedPixelOperation);
        }
        Ok(self.push(Node {
            kind: NodeKind::Resize {
                input,
                domain: ResizeDomain::EncodedSamples,
            },
            size,
            format: parent.format,
            semantics: parent.semantics.clone(),
        }))
    }

    /// Opt-in linear-light sRGB resize. No implicit color guessing or ICC
    /// parsing: a nonconflicting, explicitly tagged full-range RGB sRGB
    /// source and established alpha association are required.
    pub(crate) fn resize_linear_srgb(
        &mut self,
        input: NodeId,
        size: Size,
    ) -> Result<NodeId, Error> {
        let parent = self.node(input)?;
        let expected_alpha = match parent.format {
            PixelFormat::InterleavedU8(ChannelLayout::Rgb) => AlphaMode::Opaque,
            PixelFormat::InterleavedU8(ChannelLayout::Rgba) => AlphaMode::Straight,
            _ => return Err(Error::UnsupportedPixelOperation),
        };
        if parent.semantics.alpha != expected_alpha {
            return Err(Error::UnsupportedAlphaMode);
        }
        if parent.semantics.color.linear_srgb_readiness()
            != LinearSrgbReadiness::TransferReferenceReady
        {
            return Err(Error::UnsupportedColorEncoding);
        }
        Ok(self.push(Node {
            kind: NodeKind::Resize {
                input,
                domain: ResizeDomain::LinearSrgb,
            },
            size,
            format: parent.format,
            semantics: parent.semantics.clone(),
        }))
    }

    pub(crate) fn node(&self, id: NodeId) -> Result<&Node, Error> {
        self.nodes.get(id.0).ok_or(Error::UnknownNode)
    }

    fn push(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }
}
