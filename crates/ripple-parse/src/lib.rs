//! Repository scanning and syntax extraction. Implementation follows the core contracts.

#[cfg(test)]
mod bootstrap {
    #[test]
    fn pinned_typescript_and_tsx_grammars_load() {
        for (language, source) in [
            (
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                "export function f(): number { return 1; }",
            ),
            (
                tree_sitter_typescript::LANGUAGE_TSX,
                "export function App() { return <div>Hello</div>; }",
            ),
        ] {
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&language.into()).unwrap();
            let tree = parser.parse(source, None).unwrap();
            assert!(!tree.root_node().has_error());
        }
    }
}
