//! Stage 1 — line-based segmenter (PRS-01, PRS-02, PRS-03).
//!
//! # Invariants (load-bearing — see RESEARCH §Two-Stage Parser → Stage 1)
//!
//! 1. `RawBlock` byte ranges are **contiguous and non-overlapping** when
//!    sorted by `byte_offset`.
//! 2. Concatenating `source[block.byte_offset .. block.byte_offset +
//!    block.byte_length]` for every block in order **exactly equals**
//!    `source`. This is the splice-noop property — proves ACPT-01 / PRS-07
//!    by construction.
//! 3. A "page prelude" `RawBlock` (depth = `u8::MAX`) covers any bytes
//!    before the first bullet line (page-level `title::` properties, blank
//!    lines, etc.). If the file starts with a bullet at byte 0, the prelude
//!    block has `byte_length = 0` and exists as a placeholder.
//!
//! # Implementation
//!
//! Hand-rolled line-based state machine. Walks the source line-by-line,
//! tracking the current block, an optional open code fence inside it, and
//! an optional open drawer (`:LOGBOOK:` / `:END:`). A new bullet line is
//! ONLY recognized when neither a fence nor a drawer is open — that is what
//! keeps fenced `- foo` lines from being misclassified as siblings (the
//! exact failure mode of off-the-shelf CommonMark parsers on the Logseq
//! corpus; see RESEARCH §Don't Hand-Roll).

/// A raw block as produced by the line-based segmenter.
///
/// `raw` is the exact slice of the source covering this block, inclusive
/// of continuation lines, drawers, properties, and the trailing newline.
/// `properties` and `drawers` are parsed-out views over the same bytes —
/// the index uses them, but write-back always operates on the byte range
/// (PRS-05, PRS-06, D-05, D-06).
#[derive(Debug, Clone)]
pub struct RawBlock {
    /// TAB-indent count. `0` = top-level bullet, `1` = nested once, etc.
    /// `u8::MAX` is a sentinel for the page-prelude block.
    pub depth: u8,
    /// Absolute byte offset into the source file.
    pub byte_offset: usize,
    /// Length in bytes, inclusive of all continuation lines, drawers,
    /// properties and the trailing newline.
    pub byte_length: usize,
    /// Full raw text of the block (UTF-8). Equals
    /// `source[byte_offset..byte_offset + byte_length]`.
    pub raw: String,
    /// `key:: value` block properties found inside this block. Parsed for
    /// the index; `raw` already contains them verbatim.
    pub properties: Vec<(String, String)>,
    /// Drawers (`:LOGBOOK: ... :END:` and friends) found inside this
    /// block. Opaque blobs; byte ranges are relative to the source file.
    pub drawers: Vec<RawDrawer>,
}

/// A Logseq-style drawer. Treated as an opaque blob: we record name and
/// byte range, but never look inside (PRS-06, D-06).
#[derive(Debug, Clone)]
pub struct RawDrawer {
    /// Drawer name (e.g. `"LOGBOOK"`). Stored without surrounding colons.
    pub name: String,
    /// Absolute byte offset into the source file (start of `:NAME:` line).
    pub byte_offset: usize,
    /// Length in bytes, inclusive of the `:END:` closing line.
    pub byte_length: usize,
}

/// Sentinel depth marking the page-prelude block.
const PRELUDE_DEPTH: u8 = u8::MAX;

/// In-progress state for an open code fence inside the current block.
#[derive(Debug, Clone, Copy)]
struct FenceState {
    /// Backtick or tilde — the marker character used to open the fence.
    marker: u8,
    /// Number of marker characters used to open. A closing fence needs
    /// at least this many of the same marker.
    count: usize,
}

/// In-progress state for an open drawer inside the current block.
#[derive(Debug)]
struct DrawerState {
    name: String,
    byte_offset: usize,
}

/// Segment a markdown source buffer into raw blocks.
///
/// See module docs for invariants. Empty input returns a single prelude
/// block with `byte_length = 0`.
pub fn segment(source: &[u8]) -> Vec<RawBlock> {
    let mut blocks: Vec<RawBlock> = Vec::new();

    // Always start with a prelude block. If the file begins with a bullet
    // at byte 0, the prelude stays at byte_length = 0 (invariant #3).
    let mut current = new_prelude(0);
    let mut fence: Option<FenceState> = None;
    let mut drawer: Option<DrawerState> = None;

    let mut line_start: usize = 0;
    while line_start < source.len() {
        // Find the end of this line (exclusive of '\n'); line_end_with_nl
        // includes the newline byte if present.
        let nl_rel = source[line_start..].iter().position(|&b| b == b'\n');
        let (line_end, line_end_with_nl) = match nl_rel {
            Some(p) => (line_start + p, line_start + p + 1),
            None => (source.len(), source.len()),
        };
        let line = &source[line_start..line_end];

        // Decide: is this line a NEW bullet, or a continuation of the
        // current block?
        let starts_new_bullet =
            fence.is_none() && drawer.is_none() && detect_bullet_depth(line).is_some();

        if starts_new_bullet {
            // Emit current block (prelude or previous bullet).
            finalize(&mut current, source);
            blocks.push(current);

            let depth = detect_bullet_depth(line).expect("checked above");
            current = RawBlock {
                depth,
                byte_offset: line_start,
                byte_length: line_end_with_nl - line_start,
                raw: String::new(), // filled at finalize
                properties: Vec::new(),
                drawers: Vec::new(),
            };

            // The bullet's own line may itself open a code fence:
            //   `\t...\t- ```rust`
            // Strip leading `\t{depth}- ` (3 + depth bytes from `\t*` + `- `).
            let prefix_len = depth as usize + 2; // depth TABs + "- "
            if line.len() >= prefix_len {
                let after_dash = &line[prefix_len..];
                if let Some(f) = detect_fence_open(after_dash) {
                    fence = Some(f);
                }
            }
        } else {
            // Continuation of current block. Extend byte_length.
            current.byte_length += line_end_with_nl - line_start;

            // Fence handling first — when a fence is open, NOTHING inside it
            // is interpreted (no drawers, no properties, no nested fences).
            if let Some(f) = fence {
                // Look for the close. The close line MUST start with the
                // continuation prefix (depth TABs + "  ") then >= `f.count`
                // markers of the same kind, then optional whitespace.
                // For prelude/non-bullet blocks (depth == PRELUDE_DEPTH or
                // depth before any bullet), the prefix is empty.
                let after = strip_continuation_prefix(line, current.depth);
                if let Some(after) = after {
                    if is_fence_close(after, f) {
                        fence = None;
                    }
                }
                // else: line doesn't match expected continuation prefix;
                // treat as still inside the fence. (Some Logseq files have
                // fence content with weird leading whitespace; preserving
                // raw bytes is all that matters.)
            } else if let Some(d) = drawer.as_ref() {
                // Inside a drawer; look for :END: close.
                let after = strip_continuation_prefix(line, current.depth);
                let trimmed = after.map(trim_ascii_ws).unwrap_or(line);
                if trimmed == b":END:" {
                    // Close drawer; range covers from :NAME: line through this :END: line.
                    let drawer_end = line_end_with_nl;
                    current.drawers.push(RawDrawer {
                        name: d.name.clone(),
                        byte_offset: d.byte_offset,
                        byte_length: drawer_end - d.byte_offset,
                    });
                    drawer = None;
                }
            } else {
                // Normal continuation: check for fence open, drawer open,
                // or property line. All require the depth-appropriate
                // continuation prefix (depth TABs + "  ") for bullets, or
                // empty prefix for prelude.
                if let Some(after) = strip_continuation_prefix(line, current.depth) {
                    if let Some(f) = detect_fence_open(after) {
                        fence = Some(f);
                    } else if let Some(name) = detect_drawer_open(after) {
                        drawer = Some(DrawerState {
                            name,
                            byte_offset: line_start,
                        });
                    } else if let Some((k, v)) = detect_property(after) {
                        current.properties.push((k, v));
                    }
                }
                // If no prefix match, this is a "loose" continuation line
                // (blank line, or a line with unexpected indent). Either
                // way it sticks to the current block for round-trip purposes.
            }
        }

        line_start = line_end_with_nl;
    }

    // EOF: emit the in-progress block. If a drawer was open at EOF, close
    // it covering through EOF (T-02-04: malformed file preservation).
    if let Some(d) = drawer.take() {
        current.drawers.push(RawDrawer {
            name: d.name,
            byte_offset: d.byte_offset,
            byte_length: source.len() - d.byte_offset,
        });
    }
    finalize(&mut current, source);
    blocks.push(current);

    blocks
}

fn new_prelude(offset: usize) -> RawBlock {
    RawBlock {
        depth: PRELUDE_DEPTH,
        byte_offset: offset,
        byte_length: 0,
        raw: String::new(),
        properties: Vec::new(),
        drawers: Vec::new(),
    }
}

fn finalize(block: &mut RawBlock, source: &[u8]) {
    let slice = &source[block.byte_offset..block.byte_offset + block.byte_length];
    // Assumption A2: corpus is UTF-8. If violated, fail loudly per T-02-03.
    block.raw = std::str::from_utf8(slice)
        .expect("non-UTF-8 source — assumption A2 violated")
        .to_string();
}

/// If `line` is a bullet line (`\t* - ...`), return the TAB-depth. Otherwise
/// return None. A bullet line is:
///   - zero or more leading `\t` bytes
///   - then literal `"- "` (dash, space)
///   - then arbitrary content (may be empty after the space)
///
/// A line of just `"-\n"` (empty bullet) is also recognized because Logseq
/// emits those; we require `- ` OR a line that is exactly `-` (no content).
fn detect_bullet_depth(line: &[u8]) -> Option<u8> {
    let mut i = 0;
    while i < line.len() && line[i] == b'\t' {
        i += 1;
    }
    let depth = i;
    if depth > PRELUDE_DEPTH as usize - 1 {
        // Beyond u8 — treat as non-bullet (T-02-01, depth cap).
        return None;
    }
    let rest = &line[i..];
    // Either `- ...` or exactly `-` (empty bullet, per fixture 08).
    if rest == b"-" || rest.starts_with(b"- ") {
        Some(depth as u8)
    } else {
        None
    }
}

/// If the line starts with the continuation prefix for a block at `depth`,
/// return the slice after the prefix. The prefix is:
///   - prelude (`depth == PRELUDE_DEPTH`): empty prefix (any line continues)
///   - bullet (`depth = N`): N TABs followed by 2 spaces
///
/// For bullets, a fully-blank line (or whitespace-only line) is also accepted
/// as a continuation — round-trip requires keeping blank lines glued to the
/// preceding block.
fn strip_continuation_prefix(line: &[u8], depth: u8) -> Option<&[u8]> {
    if depth == PRELUDE_DEPTH {
        return Some(line);
    }
    let d = depth as usize;
    // Fully-empty line counts as continuation.
    if line.is_empty() {
        return Some(line);
    }
    if line.len() < d + 2 {
        // Could still be a blank-ish line.
        return None;
    }
    for i in 0..d {
        if line[i] != b'\t' {
            return None;
        }
    }
    if line[d] != b' ' || line[d + 1] != b' ' {
        return None;
    }
    Some(&line[d + 2..])
}

/// Detect a code-fence opener at the start of `s`. Returns the fence state
/// (marker char + count) if `s` starts with 3+ backticks or 3+ tildes.
fn detect_fence_open(s: &[u8]) -> Option<FenceState> {
    if s.len() < 3 {
        return None;
    }
    let m = s[0];
    if m != b'`' && m != b'~' {
        return None;
    }
    let mut count = 0usize;
    while count < s.len() && s[count] == m {
        count += 1;
    }
    if count >= 3 {
        Some(FenceState { marker: m, count })
    } else {
        None
    }
}

/// True iff `s` (already stripped of continuation prefix) is a fence-close
/// line for the open fence `f`. A closer is `>= f.count` of `f.marker`,
/// optionally surrounded by ASCII whitespace, with nothing else on the line.
fn is_fence_close(s: &[u8], f: FenceState) -> bool {
    // Trim leading/trailing ASCII whitespace (spaces/tabs).
    let s = trim_ascii_ws(s);
    if s.len() < f.count {
        return false;
    }
    s.iter().all(|&b| b == f.marker) && s.len() >= f.count
}

/// Detect a drawer-open line. Returns the drawer name (without colons) if
/// `s` is exactly `:NAME:` where NAME is one or more ASCII uppercase letters,
/// and NAME != "END".
fn detect_drawer_open(s: &[u8]) -> Option<String> {
    let s = trim_ascii_ws(s);
    if s.len() < 3 {
        return None;
    }
    if s[0] != b':' || s[s.len() - 1] != b':' {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return None;
    }
    if !inner.iter().all(|&b| b.is_ascii_uppercase()) {
        return None;
    }
    if inner == b"END" {
        return None;
    }
    Some(String::from_utf8(inner.to_vec()).expect("ASCII"))
}

/// Detect a `key:: value` property line. The key is `[A-Za-z][A-Za-z0-9._-]*`
/// followed by `:: ` then arbitrary value (which may be empty).
fn detect_property(s: &[u8]) -> Option<(String, String)> {
    if s.is_empty() {
        return None;
    }
    // First byte must be ASCII alpha.
    if !s[0].is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < s.len()
        && (s[i].is_ascii_alphanumeric() || s[i] == b'.' || s[i] == b'_' || s[i] == b'-')
    {
        i += 1;
    }
    let key_end = i;
    // Need at least `:: ` after the key (`::` followed by space) OR `::` at end.
    if i + 1 >= s.len() {
        return None;
    }
    if s[i] != b':' || s[i + 1] != b':' {
        return None;
    }
    i += 2;
    // Optional space separator. Logseq usually emits `key:: value` but
    // tolerates `key::value` too; we accept either.
    if i < s.len() && s[i] == b' ' {
        i += 1;
    }
    let key = String::from_utf8(s[..key_end].to_vec()).ok()?;
    // Trim trailing CR (Windows line endings shouldn't reach here because
    // .gitattributes forces LF, but be defensive).
    let mut value_slice = &s[i..];
    if let Some((&last, rest)) = value_slice.split_last() {
        if last == b'\r' {
            value_slice = rest;
        }
    }
    let value = String::from_utf8(value_slice.to_vec()).ok()?;
    Some((key, value))
}

fn trim_ascii_ws(s: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < s.len() && (s[start] == b' ' || s[start] == b'\t') {
        start += 1;
    }
    let mut end = s.len();
    while end > start && (s[end - 1] == b' ' || s[end - 1] == b'\t' || s[end - 1] == b'\r') {
        end -= 1;
    }
    &s[start..end]
}
