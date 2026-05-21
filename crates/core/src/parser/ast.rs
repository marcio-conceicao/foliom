//! Stage 2 — per-block CommonMark walker + ref extraction (PRS-04).
//!
//! `extract_refs(raw)` walks a block's text via the `pulldown-cmark` event
//! stream, suppressing extraction inside headings, code blocks, inline code,
//! links, and images. From the surviving `Event::Text` payloads it extracts
//! three syntaxes in strict prefix order:
//!
//! 1. `#[[multi word tag]]` — composite tag (must be tried BEFORE bare `#`).
//! 2. `[[Page Name]]` — page link.
//! 3. `#bare-tag` — bare tag; rejected if (a) preceded by an alphanumeric
//!    character (URL-fragment guard) or (b) the captured token matches a
//!    hex-color pattern `^[0-9a-fA-F]{3,8}$`.
//!
//! Targets are post-processed: `%2F` (any case) → `/`, then NFC-normalized
//! via `unicode-normalization`. This matches the way `RelativePath`
//! normalizes filenames so `#[[Avaliação]]` keys the same page regardless of
//! whether the source bytes were NFC or NFD.
//!
//! # AP-1 invariant
//!
//! `extract_refs` is called **per-block** on `RawBlock.raw`, never on a whole
//! file. The block boundaries (set by Stage 1) already excluded heading/code
//! contexts that span the file at the segmenter level; the CommonMark
//! suppression handles intra-block contexts.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_normalization::UnicodeNormalization;

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

/// Extract `[[link]]`, `#tag`, and `#[[multi word tag]]` from a block's raw
/// text. See module docs for suppression rules and target post-processing.
pub fn extract_refs(raw: &str) -> Vec<ExtractedRef> {
    // The Stage 1 segmenter passes per-block `raw` that often starts with
    // `\t...\t- ` (TAB-indent + bullet). pulldown-cmark would interpret a
    // TAB-indented line as an indented code block and suppress everything.
    // Strip a leading bullet prefix (`\t* - `) and re-indent continuation
    // lines (TAB + 2-space prefix → empty) so the parser sees plain
    // paragraph text.
    let normalized = strip_segmenter_prefix(raw);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(&normalized, opts);

    let mut out: Vec<ExtractedRef> = Vec::new();
    let mut suppress_depth: u32 = 0;
    // pulldown-cmark splits punctuation like `[` and `]` into separate Text
    // events; we need to scan the concatenation, otherwise `[[Foo]]` is six
    // separate Text events that never match `[[`. Buffer consecutive Text
    // events and flush on any structural transition.
    let mut buf = String::new();

    let flush = |buf: &mut String, out: &mut Vec<ExtractedRef>| {
        if !buf.is_empty() {
            scan_text_for_refs(buf, out);
            buf.clear();
        }
    };

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. })
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::Link { .. })
            | Event::Start(Tag::Image { .. }) => {
                flush(&mut buf, &mut out);
                suppress_depth += 1;
            }
            Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::CodeBlock)
            | Event::End(TagEnd::Link)
            | Event::End(TagEnd::Image) => {
                // Inside-suppression text was never buffered, so nothing to flush.
                suppress_depth = suppress_depth.saturating_sub(1);
            }
            Event::Text(t) if suppress_depth == 0 => {
                buf.push_str(&t);
            }
            // Inline code (`...`) — flush buffered text first (boundary), then
            // skip the code payload entirely. The code itself never contributes.
            Event::Code(_) => {
                flush(&mut buf, &mut out);
            }
            // Any other structural event ends the current text run.
            Event::Start(_) | Event::End(_) | Event::SoftBreak | Event::HardBreak => {
                // Preserve whitespace across soft/hard breaks so `#foo\n#bar`
                // (rare inside a single inline run) doesn't glue tokens.
                if matches!(event, Event::SoftBreak | Event::HardBreak) {
                    buf.push(' ');
                } else {
                    flush(&mut buf, &mut out);
                }
            }
            _ => {}
        }
    }
    flush(&mut buf, &mut out);

    out
}

/// Hand-rolled char-by-char scanner for `[[...]]`, `#[[...]]`, and `#bare`.
fn scan_text_for_refs(text: &str, out: &mut Vec<ExtractedRef>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        // `#[[...]]` composite tag must be tried BEFORE bare `#`.
        if b == b'#' && i + 2 < len && bytes[i + 1] == b'[' && bytes[i + 2] == b'[' {
            // URL-fragment guard: # must not be glued to alphanumeric
            // (`foo#[[bar]]` looks like a URL fragment; reject).
            if i > 0 && is_ascii_alphanumeric(bytes[i - 1]) {
                i += 1;
                continue;
            }
            if let Some(end) = find_double_close(bytes, i + 3) {
                let inner = &text[i + 3..end];
                if let Some(target) = canonicalize_target(inner) {
                    out.push(ExtractedRef {
                        kind: RefKind::Tag,
                        target,
                    });
                }
                i = end + 2;
                continue;
            }
            // Unterminated; skip the `#`.
            i += 1;
            continue;
        }

        // `[[...]]` page link.
        if b == b'[' && i + 1 < len && bytes[i + 1] == b'[' {
            if let Some(end) = find_double_close(bytes, i + 2) {
                let inner = &text[i + 2..end];
                if let Some(target) = canonicalize_target(inner) {
                    out.push(ExtractedRef {
                        kind: RefKind::PageLink,
                        target,
                    });
                }
                i = end + 2;
                continue;
            }
            i += 1;
            continue;
        }

        // Bare `#tag`.
        if b == b'#' {
            // URL-fragment guard: alphanumeric immediately before `#`.
            if i > 0 && is_ascii_alphanumeric(bytes[i - 1]) {
                i += 1;
                continue;
            }
            // Read tag token: first char must be alphanumeric; then
            // `alnum | - | _ | / | .`.
            let tok_start = i + 1;
            let mut j = tok_start;
            if j >= len || !is_tag_first_char(bytes[j]) {
                i += 1;
                continue;
            }
            j += 1;
            while j < len && is_tag_cont_char(bytes[j]) {
                j += 1;
            }
            // Strip trailing `.` (sentence terminator) — and only that.
            let mut tok_end = j;
            while tok_end > tok_start && bytes[tok_end - 1] == b'.' {
                tok_end -= 1;
            }
            if tok_end == tok_start {
                i += 1;
                continue;
            }
            let token = &text[tok_start..tok_end];
            // Hex-color rejection: `^[0-9a-fA-F]{3,8}$` AND no non-hex
            // disambiguator. (3,6,8 are the typical hex-color lengths;
            // tighten to 3/6/8 only to reduce false-rejects on tags like
            // `#abcd`.)
            if is_hex_color_like(token) {
                i = j;
                continue;
            }
            if let Some(target) = canonicalize_target(token) {
                out.push(ExtractedRef {
                    kind: RefKind::Tag,
                    target,
                });
            }
            i = j;
            continue;
        }

        i += 1;
    }
}

/// Find the byte index of the next `]]` at or after `start`, returning the
/// index of the first `]`. Returns None if not found.
fn find_double_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut k = start;
    while k + 1 < bytes.len() {
        if bytes[k] == b']' && bytes[k + 1] == b']' {
            return Some(k);
        }
        k += 1;
    }
    None
}

fn is_ascii_alphanumeric(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn is_tag_first_char(b: u8) -> bool {
    // First char must be alphanumeric or underscore — NOT `-` or `/` or `.`
    // (those would let `#-foo` or `#/foo` slip through; not desired).
    b.is_ascii_alphanumeric() || b == b'_' || !b.is_ascii()
}

fn is_tag_cont_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'/' || b == b'.' || !b.is_ascii()
}

/// Hex-color check: token is 3, 6, or 8 hex digits exactly.
fn is_hex_color_like(s: &str) -> bool {
    let n = s.len();
    if n != 3 && n != 6 && n != 8 {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Decode `%2F` (any case) → `/`, NFC-normalize, trim. Returns None if the
/// resulting target is empty.
fn canonicalize_target(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let decoded = decode_percent_2f(raw);
    let nfc: String = decoded.nfc().collect();
    let trimmed = nfc.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Replace `%2F` / `%2f` with `/`. Per LNK-02 we only handle this one
/// sequence, not a general percent-decoder.
fn decode_percent_2f(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 2 < bytes.len()
            && bytes[i] == b'%'
            && bytes[i + 1] == b'2'
            && (bytes[i + 2] == b'F' || bytes[i + 2] == b'f')
        {
            out.push('/');
            i += 3;
        } else {
            // Push one UTF-8 char.
            // Find next char boundary.
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&s[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

/// Strip the segmenter's structural prefix from each line so pulldown-cmark
/// sees plain paragraph text. Rules:
///   - First line: drop leading TABs then `- ` (or exactly `-`).
///   - Continuation lines: drop leading TABs then 2 spaces if present.
/// Lines that don't match are passed through verbatim.
fn strip_segmenter_prefix(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for (idx, line) in raw.split_inclusive('\n').enumerate() {
        // Split into content + (optional) trailing newline.
        let (content, nl) = match line.strip_suffix('\n') {
            Some(c) => (c, "\n"),
            None => (line, ""),
        };
        let bytes = content.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i] == b'\t' {
            i += 1;
        }
        if idx == 0 {
            // Bullet line: after TABs expect `- ` or exactly `-`.
            let rest = &bytes[i..];
            if rest == b"-" {
                out.push_str(nl);
                continue;
            }
            if rest.starts_with(b"- ") {
                out.push_str(&content[i + 2..]);
                out.push_str(nl);
                continue;
            }
        } else {
            // Continuation: after TABs expect 2 spaces.
            if bytes.len() >= i + 2 && bytes[i] == b' ' && bytes[i + 1] == b' ' {
                out.push_str(&content[i + 2..]);
                out.push_str(nl);
                continue;
            }
        }
        // Non-matching line — pass through verbatim.
        out.push_str(line);
    }
    out
}

fn utf8_char_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xC0 {
        1 // continuation byte alone — should not happen on valid UTF-8 boundary
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}
