// SPDX-License-Identifier: GPL-3.0-or-later
//! The key-mapping file's lexical syntax and node tree (`keymap.md` §12.3,
//! §12.4), with the importer's own limits and directive handling
//! (§12.14.1 step 2).

use std::fmt;

/// Deepest node nesting the importer accepts (§12.14.1; empty-text nodes
/// count as levels).
pub const MAX_NODE_DEPTH: usize = 16;
/// Longest token the importer accepts, in bytes (§12.14.1).
pub const MAX_TOKEN_LEN: usize = 4096;
/// Most nodes one file may hold (a resource bound of Tessera's: a 1 MiB file
/// written by the original holds a few thousand).
pub const MAX_NODES: usize = 262_144;
/// Most problem reports the file parser and the file reader
/// ([`read_key_map`](super::read_key_map)) each add to the caller's list;
/// later ones are dropped (the import report keeps far fewer anyway).
pub const MAX_REPORTS: usize = 1024;

/// One node: a token (its **text**, raw bytes) and its child list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// The token's bytes after quoting and escapes; empty for a node written
    /// without a token (`[ … ]`, §12.4).
    pub text: Vec<u8>,
    /// The bracketed list after the token (empty when there is none).
    pub children: Vec<Node>,
}

impl Node {
    /// A node with text and children (synthetic trees, tests).
    #[must_use]
    pub fn new(text: &str, children: Vec<Node>) -> Self {
        Self {
            text: text.as_bytes().to_vec(),
            children,
        }
    }

    /// The text as UTF-8 with invalid bytes replaced (for reports).
    #[must_use]
    pub fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.text).into_owned()
    }
}

/// Why a file is rejected while it is read as a tree (§12.3, §12.4,
/// §12.14.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// The first four bytes are all 0xAA (a binary node file, §12.3 rule 8).
    BinaryMarker,
    /// A closing bracket that is not the partner of the open one, or the end
    /// of the input (or a byte 0xFF) inside a list.
    MismatchedBrackets,
    /// A closing bracket (or a byte 0xFF) at the top level.
    UnexpectedClose,
    /// An `!error` directive, with its message.
    ErrorDirective(String),
    /// An `!include` directive: the importer does not follow it.
    Include,
    /// Nesting deeper than [`MAX_NODE_DEPTH`].
    TooDeep,
    /// A token longer than [`MAX_TOKEN_LEN`].
    TokenTooLong,
    /// More than [`MAX_NODES`] nodes.
    TooManyNodes,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryMarker => f.write_str("binary node files are not supported"),
            Self::MismatchedBrackets => f.write_str("mismatched brackets"),
            Self::UnexpectedClose => f.write_str("unexpected closing bracket at the top level"),
            Self::ErrorDirective(m) => write!(f, "!error directive: {m:?}"),
            Self::Include => f.write_str("!include is not followed by the importer"),
            Self::TooDeep => write!(f, "nodes nested deeper than {MAX_NODE_DEPTH}"),
            Self::TokenTooLong => write!(f, "a token longer than {MAX_TOKEN_LEN} bytes"),
            Self::TooManyNodes => write!(f, "more than {MAX_NODES} nodes"),
        }
    }
}

/// The C text-mode translation of §12.3 rule 1: stop at the first byte
/// 0x1A, then turn every CR LF pair into LF.
#[must_use]
pub fn text_mode(raw: &[u8]) -> Vec<u8> {
    let end = raw.iter().position(|&b| b == 0x1A).unwrap_or(raw.len());
    let raw = &raw[..end];
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\r' && raw.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    out
}

/// Parse a whole file: the binary-marker check on the raw bytes, the
/// text-mode translation, then the node tree. `reports` receives the
/// problems the original only reports (unterminated quotes, a trailing
/// `\`, `!warning` messages).
///
/// # Errors
///
/// [`TreeError`] when the file is rejected.
pub fn parse_file(raw: &[u8], reports: &mut Vec<String>) -> Result<Vec<Node>, TreeError> {
    if raw.len() >= 4 && raw[..4] == [0xAA; 4] {
        return Err(TreeError::BinaryMarker);
    }
    parse_text(&text_mode(raw), reports)
}

/// Parse already translated text (§12.3 rules 2–7, §12.4).
///
/// # Errors
///
/// [`TreeError`] when the file is rejected.
pub fn parse_text(text: &[u8], reports: &mut Vec<String>) -> Result<Vec<Node>, TreeError> {
    // A byte 0x00 ends the input (§12.3 rule 1).
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    let mut p = Parser {
        s: &text[..end],
        i: 0,
        nodes: 0,
        reports,
    };
    p.list(0, None)
}

struct Parser<'a, 'r> {
    s: &'a [u8],
    i: usize,
    nodes: usize,
    reports: &'r mut Vec<String>,
}

fn is_separator(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',')
}

fn partner(open: u8) -> Option<u8> {
    match open {
        b'[' => Some(b']'),
        b'(' => Some(b')'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

fn is_close(b: u8) -> bool {
    matches!(b, b']' | b')' | b'}')
}

impl Parser<'_, '_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn report(&mut self, msg: String) {
        if self.reports.len() < MAX_REPORTS {
            self.reports.push(msg);
        }
    }

    /// Skip separators and comments (a 0xFF inside a comment ends it).
    fn skip(&mut self) {
        while let Some(b) = self.peek() {
            if is_separator(b) {
                self.i += 1;
            } else if b == b'#' {
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == b'\n' || c == 0xFF {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// The children of a list opened by `open` (or the top level for
    /// `None`), up to and including its closing bracket.
    fn list(&mut self, depth: usize, open: Option<u8>) -> Result<Vec<Node>, TreeError> {
        let close = open.and_then(partner);
        let mut out = Vec::new();
        loop {
            self.skip();
            let Some(b) = self.peek() else {
                return match close {
                    Some(_) => Err(TreeError::MismatchedBrackets),
                    None => Ok(out),
                };
            };
            if b == 0xFF {
                // Ends the list as the end of input would, but the input has
                // not ended: always fatal (§12.3 rule 7).
                return Err(if close.is_some() {
                    TreeError::MismatchedBrackets
                } else {
                    TreeError::UnexpectedClose
                });
            }
            if is_close(b) {
                return match close {
                    Some(c) if c == b => {
                        self.i += 1;
                        Ok(out)
                    }
                    Some(_) => Err(TreeError::MismatchedBrackets),
                    None => Err(TreeError::UnexpectedClose),
                };
            }
            // The node made here is at depth `depth + 1` (top-level nodes
            // are at depth 1).
            if depth >= MAX_NODE_DEPTH {
                return Err(TreeError::TooDeep);
            }
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                return Err(TreeError::TooManyNodes);
            }
            let node = if partner(b).is_some() {
                // A node without a token (§12.4).
                self.i += 1;
                Node {
                    text: Vec::new(),
                    children: self.list(depth + 1, Some(b))?,
                }
            } else {
                let text = self.token()?;
                self.skip();
                let children = match self.peek() {
                    Some(o) if partner(o).is_some() => {
                        self.i += 1;
                        self.list(depth + 1, Some(o))?
                    }
                    _ => Vec::new(),
                };
                Node { text, children }
            };
            if let Some(kept) = self.directive(node)? {
                out.push(kept);
            }
        }
    }

    /// Directives (§12.4, importer handling §12.14.1): `Ok(None)` for a node
    /// that is not kept.
    fn directive(&mut self, node: Node) -> Result<Option<Node>, TreeError> {
        if node.text.first() != Some(&b'!') {
            return Ok(Some(node));
        }
        let message = || {
            node.children
                .first()
                .map(Node::text_lossy)
                .unwrap_or_default()
        };
        if node.text.eq_ignore_ascii_case(b"!include") {
            return Err(TreeError::Include);
        }
        if node.text.eq_ignore_ascii_case(b"!error") {
            return Err(TreeError::ErrorDirective(message()));
        }
        if node.text.eq_ignore_ascii_case(b"!warning") {
            let m = message();
            self.report(format!("!warning: {m:?}"));
            return Ok(None);
        }
        Ok(Some(node))
    }

    fn push_token_byte(&self, out: &mut Vec<u8>, b: u8) -> Result<(), TreeError> {
        if out.len() >= MAX_TOKEN_LEN {
            return Err(TreeError::TokenTooLong);
        }
        out.push(b);
        Ok(())
    }

    fn token(&mut self) -> Result<Vec<u8>, TreeError> {
        let mut out = Vec::new();
        if self.peek() == Some(b'"') {
            self.i += 1;
            loop {
                match self.peek() {
                    None | Some(b'\n' | 0xFF) => {
                        self.report(format!("unterminated quoted token at byte {}", self.i));
                        return Ok(out);
                    }
                    Some(b'"') => {
                        self.i += 1;
                        return Ok(out);
                    }
                    Some(b'\\') => {
                        self.i += 1;
                        match self.peek() {
                            Some(c) => {
                                self.i += 1;
                                self.push_token_byte(&mut out, c)?;
                            }
                            None => {
                                self.report("`\\` at the end of the input".into());
                                return Ok(out);
                            }
                        }
                    }
                    Some(c) => {
                        self.i += 1;
                        self.push_token_byte(&mut out, c)?;
                    }
                }
            }
        }
        while let Some(b) = self.peek() {
            if is_separator(b) || b == b'"' || b == b'#' || b == 0xFF || is_close(b) {
                break;
            }
            if partner(b).is_some() {
                break;
            }
            self.i += 1;
            if b == b'\\' {
                match self.peek() {
                    Some(0xFF) => {
                        self.i += 1;
                        self.report("`\\` before a byte 0xFF".into());
                        break;
                    }
                    None => {
                        self.report("`\\` at the end of the input".into());
                        break;
                    }
                    Some(c) => {
                        self.i += 1;
                        self.push_token_byte(&mut out, c)?;
                    }
                }
            } else {
                self.push_token_byte(&mut out, b)?;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(s: &[u8]) -> Result<Vec<Node>, TreeError> {
        parse_file(s, &mut Vec::new())
    }

    fn n(t: &str, c: Vec<Node>) -> Node {
        Node::new(t, c)
    }

    #[test]
    fn example_a_tokens_and_tree() {
        let t = tree(b"CameraReset [ Old [ 0 DIK_NUMPAD0 ] ] # note").unwrap();
        assert_eq!(
            t,
            [n(
                "CameraReset",
                vec![n("Old", vec![n("0", vec![]), n("DIK_NUMPAD0", vec![])])]
            )]
        );
    }

    #[test]
    fn example_b_quoting() {
        assert_eq!(tree(br#""a \"b\" c""#).unwrap(), [n("a \"b\" c", vec![])]);
        assert_eq!(tree(br"x\#y").unwrap(), [n("x#y", vec![])]);
        assert_eq!(
            tree(b"DIJOFS_BUTTON(5)").unwrap(),
            [n("DIJOFS_BUTTON", vec![n("5", vec![])])]
        );
        assert_eq!(
            tree(br#""DIJOFS_BUTTON(5)""#).unwrap(),
            [n("DIJOFS_BUTTON(5)", vec![])]
        );
        assert_eq!(tree(b"a,b").unwrap(), [n("a", vec![]), n("b", vec![])]);
        assert_eq!(tree(br#""""#).unwrap(), [n("", vec![])]);
    }

    #[test]
    fn example_h_nodes_without_tokens_and_raw_bytes() {
        assert_eq!(
            tree(b"A [ x ] [ y ]").unwrap(),
            [n("A", vec![n("x", vec![])]), n("", vec![n("y", vec![])])]
        );
        assert_eq!(
            tree(b"A [ [ x ] ]").unwrap(),
            [n("A", vec![n("", vec![n("x", vec![])])])]
        );
        assert_eq!(
            tree(b"A B [ x ]").unwrap(),
            [n("A", vec![]), n("B", vec![n("x", vec![])])]
        );
        // `ab\`, CR LF, `cd`: one token of 5 bytes with the translation.
        let t = tree(&[0x61, 0x62, 0x5C, 0x0D, 0x0A, 0x63, 0x64]).unwrap();
        assert_eq!(t[0].text, [0x61, 0x62, 0x0A, 0x63, 0x64]);
        let raw = parse_text(&[0x61, 0x62, 0x5C, 0x0D, 0x0A, 0x63, 0x64], &mut Vec::new()).unwrap();
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0].text, [0x61, 0x62, 0x0D]);
        // `"ab`, CR LF: unterminated, reported.
        let mut r = Vec::new();
        let t = parse_file(&[0x22, 0x61, 0x62, 0x0D, 0x0A], &mut r).unwrap();
        assert_eq!(t[0].text, b"ab");
        assert_eq!(r.len(), 1);
        // `A [ x`, 0xFF, ` ]`.
        assert_eq!(
            tree(&[0x41, 0x20, 0x5B, 0x20, 0x78, 0xFF, 0x20, 0x5D]),
            Err(TreeError::MismatchedBrackets)
        );
        assert_eq!(tree(&[0xFF]), Err(TreeError::UnexpectedClose));
        // 0xFF ends a comment as LF would.
        assert_eq!(tree(b"# \xFF comment\nA").unwrap().len(), 2);
        assert_eq!(tree(&[0xAA; 4]), Err(TreeError::BinaryMarker));
        assert_eq!(tree(&[0xAA; 3]).unwrap()[0].text, [0xAA; 3]);
        // Bytes after 0x1A and 0x00 are not read.
        assert_eq!(tree(b"A\x1a ]").unwrap().len(), 1);
        assert_eq!(tree(b"A\x00 ]").unwrap().len(), 1);
    }

    #[test]
    fn brackets_and_limits() {
        assert_eq!(tree(b"A [ x )"), Err(TreeError::MismatchedBrackets));
        assert_eq!(tree(b"A [ x"), Err(TreeError::MismatchedBrackets));
        assert_eq!(tree(b"]"), Err(TreeError::UnexpectedClose));
        assert_eq!(tree(b"A ( x ) B { y }").unwrap().len(), 2);
        let deep = "[".repeat(MAX_NODE_DEPTH + 1) + &"]".repeat(MAX_NODE_DEPTH + 1);
        assert_eq!(tree(deep.as_bytes()), Err(TreeError::TooDeep));
        let ok = "[".repeat(MAX_NODE_DEPTH) + &"]".repeat(MAX_NODE_DEPTH);
        assert!(tree(ok.as_bytes()).is_ok());
        let long = "a".repeat(MAX_TOKEN_LEN + 1);
        assert_eq!(tree(long.as_bytes()), Err(TreeError::TokenTooLong));
    }

    #[test]
    fn directives() {
        let mut r = Vec::new();
        let t = parse_file(br#""!WARNING" [ note ] A"#, &mut r).unwrap();
        assert_eq!(t, [n("A", vec![])]);
        assert_eq!(r, ["!warning: \"note\""]);
        assert_eq!(
            tree(b"A [ \\!Error ]"),
            Err(TreeError::ErrorDirective(String::new()))
        );
        assert_eq!(tree(b"!include [ x ]"), Err(TreeError::Include));
        assert_eq!(tree(b"!foo [ x ]").unwrap()[0].text, b"!foo");
        assert_eq!(tree(b"!errorx").unwrap().len(), 1);
        // A directive's own list must still be well formed.
        assert_eq!(tree(b"!warning [ x )"), Err(TreeError::MismatchedBrackets));
    }
}
