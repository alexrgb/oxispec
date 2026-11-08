//! Finding the piece of code that an annotation is about, and a hash of it.
//!
//! A `// spec: REQ-1` comment sits above a function, a type, a test. With a
//! tree-sitter parser we know which one, and we can hash its body without
//! comments, blank lines or indentation. When the hash changes, the code
//! changed in a way that can matter to the requirement.
//!
//! Go, Rust, TypeScript and JavaScript are supported. For other languages
//! there is no symbol, and a link is only watched on the spec side.

use std::path::Path;

use serde::Serialize;
use tree_sitter::{Language, Node, Parser};

/// The code an annotation is attached to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symbol {
    /// The name, or for a test call its title. `impl Foo` for an impl block.
    pub name: String,
    /// The tree-sitter kind, for example `function_item`.
    pub kind: String,
    /// First and last line, 1-based.
    pub start_line: usize,
    pub end_line: usize,
    /// Hash of the code, without comments and indentation.
    pub hash: String,
    /// The names of the fields, for a struct, class, interface or object type. These are
    /// what an entity's attributes are looked for in. Empty for other code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Go,
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
}

fn language_of(path: &Path) -> Option<Lang> {
    match path.extension()?.to_str()? {
        "go" => Some(Lang::Go),
        "rs" => Some(Lang::Rust),
        "ts" | "mts" | "cts" => Some(Lang::TypeScript),
        "tsx" => Some(Lang::Tsx),
        "js" | "jsx" | "mjs" | "cjs" => Some(Lang::JavaScript),
        _ => None,
    }
}

fn grammar(lang: Lang) -> Language {
    match lang {
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
    }
}

/// Node kinds that count as a symbol an annotation can be about.
fn is_symbol(lang: Lang, node: Node) -> bool {
    let kind = node.kind();
    match lang {
        Lang::Rust => matches!(
            kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "mod_item"
                | "const_item"
                | "static_item"
                | "type_item"
                | "union_item"
                | "macro_definition"
        ),
        Lang::Go => matches!(
            kind,
            "function_declaration"
                | "method_declaration"
                | "type_declaration"
                | "const_declaration"
                | "var_declaration"
        ),
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => {
            matches!(
                kind,
                "function_declaration"
                    | "generator_function_declaration"
                    | "class_declaration"
                    | "method_definition"
                    | "interface_declaration"
                    | "type_alias_declaration"
                    | "enum_declaration"
                    | "lexical_declaration"
                    | "variable_declaration"
            ) || is_test_call(node)
        }
    }
}

/// `it('title', ...)`, `test(...)`, `describe(...)` as a statement.
fn is_test_call(node: Node) -> bool {
    node.kind() == "expression_statement"
        && node
            .named_child(0)
            .is_some_and(|call| call.kind() == "call_expression")
}

/// Symbols whose members are symbols too.
fn is_container(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "impl_item" | "trait_item" | "mod_item" | "class_declaration" => true,
        "expression_statement" => node
            .named_child(0)
            .and_then(|call| call.child_by_field_name("function"))
            .is_some_and(|f| matches!(text(f, source), "describe" | "context" | "suite")),
        _ => false,
    }
}

fn text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// A short name for the symbol.
fn name_of(lang: Lang, node: Node, source: &[u8]) -> String {
    if let Some(name) = node.child_by_field_name("name") {
        return text(name, source).to_string();
    }
    match node.kind() {
        "impl_item" => {
            let ty = node
                .child_by_field_name("type")
                .map_or("", |t| text(t, source));
            return format!("impl {ty}");
        }
        "type_declaration" | "const_declaration" | "var_declaration" => {
            // Go: the name is on the first spec inside.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(name) = child.child_by_field_name("name") {
                    return text(name, source).to_string();
                }
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(name) = child.child_by_field_name("name") {
                    return text(name, source).to_string();
                }
            }
        }
        "expression_statement" if lang != Lang::Go => {
            if let Some(title) = test_title(node, source) {
                return title;
            }
        }
        _ => {}
    }
    node.kind().to_string()
}

/// The first string argument of a test call, as the title.
fn test_title(node: Node, source: &[u8]) -> Option<String> {
    let call = node.named_child(0)?;
    let callee = call
        .child_by_field_name("function")
        .map_or("", |f| text(f, source));
    let args = call.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    let first = args
        .named_children(&mut cursor)
        .find(|a| matches!(a.kind(), "string" | "template_string"))?;
    let title = text(first, source).trim_matches(['"', '\'', '`']);
    Some(format!("{callee} {title}"))
}

/// The source of a node with comments taken out, and every line trimmed
/// with blank lines dropped, so reformatting and comment edits do not count.
fn normalized(node: Node, source: &[u8]) -> String {
    let (start, end) = (node.start_byte(), node.end_byte());
    let mut comments: Vec<(usize, usize)> = Vec::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind().contains("comment") {
            comments.push((n.start_byte().max(start), n.end_byte().min(end)));
            continue;
        }
        let mut cursor = n.walk();
        stack.extend(n.children(&mut cursor));
    }
    comments.sort_unstable();
    let mut kept = Vec::with_capacity(end - start);
    let mut at = start;
    for (from, to) in comments {
        if from > at {
            kept.extend_from_slice(&source[at..from]);
        }
        at = at.max(to);
    }
    kept.extend_from_slice(&source[at..end]);
    String::from_utf8_lossy(&kept)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The names of the members that hold data, for the kinds of code that define a type.
fn fields_of(node: Node, source: &[u8]) -> Vec<String> {
    // Rust: `struct S { a: T }`. Go: `type S struct { a T }`.
    // TypeScript: `interface S { a: T }`, `class S { a: T }`, `type S = { a: T }`.
    // JavaScript: `class S { a = 1 }`.
    let mut bodies: Vec<Node> = Vec::new();
    match node.kind() {
        "struct_item" | "interface_declaration" | "class_declaration" => {
            bodies.extend(node.child_by_field_name("body"));
        }
        "type_alias_declaration" => {
            bodies.extend(node.child_by_field_name("value"));
        }
        "type_declaration" => {
            let mut cursor = node.walk();
            for spec in node.named_children(&mut cursor) {
                if let Some(ty) = spec.child_by_field_name("type")
                    && ty.kind() == "struct_type"
                {
                    let mut inner = ty.walk();
                    bodies.extend(
                        ty.named_children(&mut inner)
                            .filter(|c| c.kind() == "field_declaration_list"),
                    );
                }
            }
        }
        _ => {}
    }
    let mut names = Vec::new();
    for body in bodies {
        let mut cursor = body.walk();
        for member in body.named_children(&mut cursor) {
            if !matches!(
                member.kind(),
                "field_declaration"
                    | "property_signature"
                    | "public_field_definition"
                    | "field_definition"
            ) {
                continue;
            }
            // Go can name several fields in one declaration.
            let mut inner = member.walk();
            let named: Vec<Node> = member
                .children_by_field_name("name", &mut inner)
                .chain(member.child_by_field_name("property"))
                .collect();
            for name in named {
                let name = text(name, source).trim_matches(['"', '\'']).to_string();
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
    }
    names
}

fn symbol_of(lang: Lang, node: Node, source: &[u8]) -> Symbol {
    Symbol {
        fields: fields_of(node, source),
        name: name_of(lang, node, source),
        kind: node.kind().to_string(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        hash: blake3::hash(normalized(node, source).as_bytes())
            .to_hex()
            .to_string(),
    }
}

/// Every symbol node in the tree, in document order.
fn symbols<'a>(lang: Lang, root: Node<'a>) -> Vec<Node<'a>> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if is_symbol(lang, node) {
            found.push(node);
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    found
}

/// The byte ranges of the comments in a file, for a language with a parser.
/// `None` for another language, or when the file cannot be parsed at all. A
/// marker in a string, a raw string or a character is not in any range.
pub fn comment_ranges(path: &Path, text: &str) -> Option<Vec<(usize, usize)>> {
    let lang = language_of(path)?;
    let mut parser = Parser::new();
    parser.set_language(&grammar(lang)).ok()?;
    let tree = parser.parse(text, None)?;
    let mut found = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind().ends_with("comment") {
            found.push((node.start_byte(), node.end_byte()));
            continue;
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children);
    }
    found.sort_unstable();
    Some(found)
}

/// The symbol for each line in `lines` (1-based). A line inside a symbol
/// gets the innermost one around it. A line above one, as a comment is, gets
/// the next symbol after it. `None` for a language without a parser, or when
/// nothing fits.
pub fn bind(path: &Path, text: &str, lines: &[usize]) -> Vec<Option<Symbol>> {
    let none = || vec![None; lines.len()];
    let Some(lang) = language_of(path) else {
        return none();
    };
    let mut parser = Parser::new();
    if parser.set_language(&grammar(lang)).is_err() {
        return none();
    }
    let Some(tree) = parser.parse(text, None) else {
        return none();
    };
    let source = text.as_bytes();
    let found = symbols(lang, tree.root_node());

    lines
        .iter()
        .map(|&line| {
            let row = line - 1;
            let around = found
                .iter()
                .filter(|n| n.start_position().row <= row && row <= n.end_position().row)
                .max_by_key(|n| n.start_byte());
            let next = |inside: Option<&Node>| {
                found
                    .iter()
                    .filter(|n| n.start_position().row > row)
                    .filter(|n| inside.is_none_or(|c| n.end_byte() <= c.end_byte()))
                    .min_by_key(|n| (n.start_position().row, n.start_byte()))
            };
            // A comment above a member of an impl, class or `describe` is about
            // the member. In a function body it is about the function.
            let chosen = match around {
                Some(a) if is_container(*a, source) => next(Some(a)).or(Some(a)),
                Some(a) => Some(a),
                None => next(None),
            };
            chosen.map(|n| symbol_of(lang, *n, source))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(path: &str, text: &str, line: usize) -> Option<Symbol> {
        bind(Path::new(path), text, &[line]).pop().unwrap()
    }

    fn name(path: &str, text: &str, line: usize) -> String {
        one(path, text, line).map_or("-".to_string(), |s| s.name)
    }

    #[test]
    fn rust_comments_bind_to_the_next_item_and_names_are_found() {
        let text = "use x;\n\n/// spec: REQ-A-0001\n#[inline]\nfn expire() {\n    work();\n}\n\n// spec: REQ-A-0002\nimpl Session {\n    // spec: REQ-A-0003\n    fn inner(&self) {}\n}\n\n// spec: REQ-A-0004\nstruct Token;\n";
        assert_eq!(name("a.rs", text, 3), "expire");
        assert_eq!(name("a.rs", text, 9), "impl Session");
        // Inside an impl, the comment above a method binds to the method.
        assert_eq!(name("a.rs", text, 11), "inner");
        assert_eq!(name("a.rs", text, 15), "Token");
        // A comment in a body binds to the function around it.
        let body = "fn f() {\n    // spec: REQ-A-0001\n    g();\n}\n";
        assert_eq!(name("a.rs", body, 2), "f");
        // Nothing follows.
        assert_eq!(name("a.rs", "fn f() {}\n// spec: REQ-A-0001\n", 2), "-");
    }

    #[test]
    fn go_and_typescript_symbols() {
        let go = "package a\n\n// spec: REQ-A-0001\nfunc Expire() {}\n\n// verifies: REQ-A-0001.S1\nfunc TestIdle(t *testing.T) {}\n\n// spec: REQ-A-0002\ntype Session struct{}\n\n// spec: REQ-A-0003\nfunc (s *Session) Close() {}\n";
        assert_eq!(name("a.go", go, 3), "Expire");
        assert_eq!(name("a_test.go", go, 6), "TestIdle");
        assert_eq!(name("a.go", go, 9), "Session");
        assert_eq!(name("a.go", go, 12), "Close");

        let ts = "// spec: REQ-A-0001\nexport function expire() {}\n\n// spec: REQ-A-0002\nexport const handler = () => {};\n\n// verifies: REQ-A-0001.S1\nit('ends idle sessions', () => {\n  expect(1).toBe(1);\n});\n\n// spec: REQ-A-0003\nclass Session {\n  // spec: REQ-A-0004\n  close() {}\n}\n";
        assert_eq!(name("a.ts", ts, 1), "expire");
        assert_eq!(name("a.ts", ts, 4), "handler");
        assert_eq!(name("a.test.ts", ts, 7), "it ends idle sessions");
        assert_eq!(name("a.ts", ts, 12), "Session");
        assert_eq!(name("a.ts", ts, 14), "close");
        assert_eq!(
            name("a.js", "// spec: REQ-A-0001\nfunction f() {}\n", 1),
            "f"
        );
        assert_eq!(
            name(
                "a.tsx",
                "// spec: REQ-A-0001\nfunction C() { return <div/>; }\n",
                1
            ),
            "C"
        );
    }

    #[test]
    fn a_bracketed_scenario_inside_a_test_binds_to_the_test() {
        let ts = "describe('auth', () => {\n  it('ends idle sessions [REQ-A-0001.S1]', () => {\n    run();\n  });\n});\n";
        // Line 2 is inside both `describe` and `it`: the innermost wins.
        assert_eq!(
            name("a.test.ts", ts, 2),
            "it ends idle sessions [REQ-A-0001.S1]"
        );
    }

    #[test]
    fn the_hash_ignores_comments_indentation_and_blank_lines_but_not_code() {
        let base = "// spec: REQ-A-0001\nfn f() {\n    let a = 1;\n    g(a);\n}\n";
        let hash = |text: &str| one("a.rs", text, 1).unwrap().hash;
        let h = hash(base);

        let reformatted =
            "// spec: REQ-A-0001\nfn f() {\n\n        let a = 1;   // note\n\tg(a);\n\n}\n";
        assert_eq!(hash(reformatted), h);
        let commented =
            "// spec: REQ-A-0001\nfn f() {\n    /* about a */ let a = 1;\n    g(a);\n}\n";
        assert_eq!(hash(commented), h);
        // The comment above and the position in the file do not matter.
        let moved = "use x;\nuse y;\n\n// spec: REQ-A-0001 (edited)\nfn f() {\n    let a = 1;\n    g(a);\n}\n";
        assert_eq!(one("a.rs", moved, 4).unwrap().hash, h);

        for changed in [
            "// spec: REQ-A-0001\nfn f() {\n    let a = 2;\n    g(a);\n}\n",
            "// spec: REQ-A-0001\nfn f() {\n    let a = 1;\n    h(a);\n}\n",
            "// spec: REQ-A-0001\nfn f(x: i32) {\n    let a = 1;\n    g(a);\n}\n",
            "// spec: REQ-A-0001\nfn f() {\n    let a = \"x  y\";\n    g(a);\n}\n",
        ] {
            assert_ne!(hash(changed), h, "{changed}");
        }
        assert_ne!(
            hash("// spec: REQ-A-0001\nfn f() {\n    let a = \"x y\";\n    g(a);\n}\n"),
            hash("// spec: REQ-A-0001\nfn f() {\n    let a = \"x  y\";\n    g(a);\n}\n"),
            "spaces inside a string are code"
        );
    }

    #[test]
    fn lines_and_kinds_are_reported_and_other_languages_have_no_symbol() {
        let s = one("a.rs", "// spec: REQ-A-0001\nfn f() {\n    g();\n}\n", 1).unwrap();
        assert_eq!((s.start_line, s.end_line), (2, 4));
        assert_eq!(s.kind, "function_item");
        assert!(one("a.py", "# spec: REQ-A-0001\ndef f(): pass\n", 1).is_none());
        assert!(one("a.txt", "// spec: REQ-A-0001\n", 1).is_none());
        // A file that does not parse still gives an answer, never a panic.
        let _ = one("a.rs", "fn ((( {{{\n// spec: REQ-A-0001\n", 2);
    }

    #[test]
    fn several_lines_in_one_file_are_bound_with_one_parse() {
        let text = "// spec: REQ-A-0001\nfn a() {}\n\n// spec: REQ-A-0002\nfn b() {}\n";
        let bound = bind(Path::new("a.rs"), text, &[1, 4]);
        let names: Vec<_> = bound
            .iter()
            .map(|s| s.as_ref().unwrap().name.as_str())
            .collect();
        assert_eq!(names, ["a", "b"]);
    }
}

#[cfg(test)]
mod tests_of_fields {
    use super::*;

    fn fields(path: &str, text: &str, line: usize) -> Vec<String> {
        bind(Path::new(path), text, &[line])
            .pop()
            .unwrap()
            .map_or(Vec::new(), |s| s.fields)
    }

    // verifies: REQ-ENTITY-0007.S3
    #[test]
    fn the_fields_of_a_type_are_found_in_each_language() {
        let rust = "// spec: ENT-0001\nstruct Order {\n    total: i64,\n    pub status: String,\n}\n\n// spec: ENT-0001\nstruct Unit;\n\n// spec: ENT-0001\nstruct Pair(i64, i64);\n";
        assert_eq!(fields("a.rs", rust, 1), ["total", "status"]);
        assert!(fields("a.rs", rust, 7).is_empty());
        assert!(fields("a.rs", rust, 10).is_empty());

        let go =
            "// spec: ENT-0001\ntype Order struct {\n\tTotal int\n\tA, B string\n\tio.Reader\n}\n";
        assert_eq!(fields("a.go", go, 1), ["Total", "A", "B"]);

        let ts = "// spec: ENT-0001\ninterface Order {\n  total: number;\n  status?: string;\n}\n\n// spec: ENT-0001\nclass Cart {\n  items: string[] = [];\n  private note = '';\n  add() {}\n}\n\n// spec: ENT-0001\ntype Line = {\n  sku: string;\n};\n";
        assert_eq!(fields("a.ts", ts, 1), ["total", "status"]);
        assert_eq!(fields("a.ts", ts, 7), ["items", "note"]);
        assert_eq!(fields("a.ts", ts, 14), ["sku"]);

        let js =
            "// spec: ENT-0001\nclass Order {\n  total = 0;\n  status = 'open';\n  pay() {}\n}\n";
        assert_eq!(fields("a.js", js, 1), ["total", "status"]);

        // A function has no fields.
        let func = "// spec: ENT-0001\nfn pay() {\n    let total = 1;\n}\n";
        assert!(fields("a.rs", func, 1).is_empty());
    }
}
