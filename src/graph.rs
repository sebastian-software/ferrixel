//! A small internal semantic graph used to validate planning contracts.
//!
//! Every new node may only refer to an already-existing node. This keeps the
//! initial graph immutable after construction and makes cycles impossible.

use crate::error::Error;
use crate::geometry::{Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeId(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Deterministic, coordinate-addressable synthetic source.
    Synthetic,
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
        })
    }

    pub(crate) fn crop(&mut self, input: NodeId, rect: Rect) -> Result<NodeId, Error> {
        if !rect.fits(self.node(input)?.size) {
            return Err(Error::OutOfBounds);
        }
        Ok(self.push(Node {
            kind: NodeKind::Crop { input, rect },
            size: Size::new(rect.width, rect.height)?,
        }))
    }

    pub(crate) fn resize(&mut self, input: NodeId, size: Size) -> Result<NodeId, Error> {
        self.node(input)?;
        Ok(self.push(Node {
            kind: NodeKind::Resize { input },
            size,
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
