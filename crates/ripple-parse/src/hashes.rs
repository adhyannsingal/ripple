//! Versioned, length-framed CST hashes. These are supported-syntax fingerprints,
//! not proof of TypeScript semantic equivalence.
use ripple_core::syntax::{Language, VersionedHash};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt::Write};
use tree_sitter::Node;

pub const SYNTAX_HASH_VERSION: u32 = 1;
/// Index consumers must use a syntax compatibility token, not just the package version.
pub const EXTRACTOR_VERSION: &str = "ripple-symbols-v2-ts0.23.2-runtime0.27.1";
pub(crate) const MAX_SYNTAX_NODES: usize = 1_000_000;

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}
// Some pinned grammar productions omit restricted line terminators inside comments.
// Retain only boundaries with JS restricted-line-break meaning, not every source newline.
fn restricted_line_break(previous: Node<'_>, current: Node<'_>, source: &str) -> bool {
    let Some(gap) = source.get(previous.end_byte()..current.start_byte()) else {
        return false;
    };
    if !gap.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return false;
    }
    let in_same_construct = |parent: Node<'_>, child: Node<'_>| {
        parent.start_byte() <= child.start_byte() && child.end_byte() <= parent.end_byte()
    };
    if !previous.is_named()
        && matches!(
            previous.kind(),
            "return" | "throw" | "yield" | "break" | "continue" | "async"
        )
        && previous
            .parent()
            .is_some_and(|p| in_same_construct(p, current))
    {
        return true;
    }
    matches!(current.kind(), "++" | "--")
        && current.parent().is_some_and(|p| {
            p.kind() == "update_expression"
                && p.start_byte() < current.start_byte()
                && in_same_construct(p, previous)
        })
}
/// Excluded nodes are implementation bodies within signatures/class shapes.
/// Node IDs are used only for selection; they never enter the hash encoding.
pub(crate) fn fingerprint(
    domain: &str,
    language: Language,
    source: &str,
    roots: &[Node<'_>],
    excluded: &BTreeSet<usize>,
    metadata: &[String],
) -> Option<VersionedHash> {
    let mut hash = Sha256::new();
    frame(&mut hash, b"ripple.syntax");
    frame(&mut hash, &SYNTAX_HASH_VERSION.to_be_bytes());
    frame(&mut hash, domain.as_bytes());
    frame(
        &mut hash,
        match language {
            Language::TypeScript => b"ts",
            Language::Tsx => b"tsx",
        },
    );
    frame(&mut hash, &(metadata.len() as u64).to_be_bytes());
    for item in metadata {
        frame(&mut hash, item.as_bytes());
    }
    frame(&mut hash, &(roots.len() as u64).to_be_bytes());
    let mut visited = 0;
    for root in roots {
        if root.has_error() {
            return None;
        }
        frame(&mut hash, b"tree");
        let mut cursor = root.walk();
        let mut previous_leaf = None;
        'tree: loop {
            let node = cursor.node();
            let skip = excluded.contains(&node.id()) || matches!(node.kind(), "comment" | ";");
            if !skip {
                visited += 1;
                if visited > MAX_SYNTAX_NODES {
                    return None;
                }
                frame(&mut hash, b"node");
                frame(
                    &mut hash,
                    if node.id() == root.id() {
                        ""
                    } else {
                        cursor.field_name().unwrap_or("")
                    }
                    .as_bytes(),
                );
                frame(&mut hash, node.kind().as_bytes());
                frame(&mut hash, &[u8::from(node.is_named())]);
                if node.child_count() == 0 {
                    frame(&mut hash, b"leaf");
                    let restricted =
                        previous_leaf.is_some_and(|p| restricted_line_break(p, node, source));
                    frame(&mut hash, &[u8::from(restricted)]);
                    frame(&mut hash, source.get(node.byte_range())?.as_bytes());
                    previous_leaf = Some(node);
                } else {
                    frame(&mut hash, b"children");
                    if cursor.goto_first_child() {
                        continue;
                    }
                }
                frame(&mut hash, b"/node");
            }
            if excluded.contains(&node.id()) || node.kind() == ";" {
                previous_leaf = None;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    break 'tree;
                }
                frame(&mut hash, b"/node");
            }
        }
        frame(&mut hash, b"/tree");
    }
    let mut digest = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut digest, "{byte:02x}").expect("String formatting cannot fail");
    }
    Some(VersionedHash {
        version: SYNTAX_HASH_VERSION,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use ripple_core::identity::RepoPath;
    #[test]
    fn v1_encoding_matches_an_independent_hand_authored_cst_digest() {
        // Expected SHA-256 computed independently from the documented frames:
        // block { return_statement(return, number(7)) }; semicolon excluded.
        let text = "function f(){return 7;}";
        let parsed = parse(RepoPath::new("fixture.ts").unwrap(), text.as_bytes()).unwrap();
        let body = parsed
            .tree()
            .unwrap()
            .root_node()
            .named_child(0)
            .unwrap()
            .child_by_field_name("body")
            .unwrap();
        let digest = fingerprint(
            "body",
            Language::TypeScript,
            text,
            &[body],
            &BTreeSet::new(),
            &[],
        )
        .unwrap();
        assert_eq!(
            digest.digest,
            "b3c312aa8168d02f65490c76d74a25b76a60314346b3c29434eddd78d9cb0145"
        );
        assert_eq!(digest.version, 1);
        let first = fingerprint(
            "signature",
            Language::TypeScript,
            text,
            &[body],
            &BTreeSet::new(),
            &["ab".into(), "c".into()],
        );
        let second = fingerprint(
            "signature",
            Language::TypeScript,
            text,
            &[body],
            &BTreeSet::new(),
            &["a".into(), "bc".into()],
        );
        assert_ne!(first, second);
        assert_ne!(
            Some(digest),
            fingerprint(
                "signature",
                Language::TypeScript,
                text,
                &[body],
                &BTreeSet::new(),
                &[]
            )
        );
    }
}
