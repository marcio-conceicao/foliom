//! Stage 2 — per-block CommonMark walker + ref extraction (PRS-04).
//!
//! Stub. Real impl follows in the GREEN gate of plan 01-03.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    PageLink,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedRef {
    pub kind: RefKind,
    pub target: String,
}

pub fn extract_refs(_raw: &str) -> Vec<ExtractedRef> {
    todo!("plan 01-03 GREEN gate")
}
