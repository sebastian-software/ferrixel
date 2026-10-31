//! A small internal semantic graph used to validate planning contracts.
//!
//! Every new node may only refer to an already-existing node. This keeps the
//! initial graph immutable after construction and makes cycles impossible.

use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::surface::{ColorMetadata, PixelFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeId(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Deterministic, coordinate-addressable synthetic source.
    Synthetic,
    Planar10Source,
    Crop {
        input: NodeId,
        rect: Rect,
    },
    /// Bilinear, pixel-center resize in Gray8 for the experiment only.
    Resize {
        input: NodeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub size: Size,
    pub format: PixelFormat,
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
        })
    }

    /// Register a semantic 10-bit source. Its external plane storage is bound
    /// separately when the executor runs; the graph never owns input buffers.
    pub(crate) fn planar_10(&mut self, size: Size, color: ColorMetadata) -> NodeId {
        self.push(Node {
            kind: NodeKind::Planar10Source,
            size,
            format: PixelFormat::Yuv420P10(color),
        })
    }

    pub(crate) fn crop(&mut self, input: NodeId, rect: Rect) -> Result<NodeId, Error> {
        let parent = self.node(input)?;
        if !rect.fits(parent.size) {
            return Err(Error::OutOfBounds);
        }
        if matches!(parent.format, PixelFormat::Yuv420P10(_))
            && (rect.x % 2 != 0 || rect.y % 2 != 0)
        {
            return Err(Error::ChromaPhaseRequiresResampling);
        }
        Ok(self.push(Node {
            kind: NodeKind::Crop { input, rect },
            size: Size::new(rect.width, rect.height)?,
            format: parent.format,
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
            kind: NodeKind::Resize { input },
            size,
            format: parent.format,
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
