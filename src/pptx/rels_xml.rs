//! RELS_XML module — provides the `rels_xml` content type.
//!
//! The actual relationships XML generation is in `slide_xml.rs`
//! (via `PresentationRelsXml`, `SlideXml` etc). This module
//! exists for structural completeness per the spec.

// Re-export everything from slide_xml for convenience.
pub use super::slide_xml::{
    PresentationRelsXml,
    RELS_DOT_RELS,
    SLIDE_MASTER_RELS_XML,
    SLIDE_LAYOUT_RELS_XML,
};
