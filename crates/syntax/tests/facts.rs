//! The same small program in each language must yield the same facts.

use syntax::{Body, FileFacts, Function, parse};

fn facts(path: &str, source: &str) -> FileFacts {
    parse(path, source).expect("supported language")
}

fn function<'a>(facts: &'a FileFacts, name: &str) -> &'a Function {
    facts
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no function {name}"))
}

fn forward_of(f: &Function) -> (String, String, Vec<String>) {
    match &f.body {
        Body::Forward(call) => (
            call.receiver.clone(),
            call.callee.clone(),
            call.args.iter().map(|a| a.text.clone()).collect(),
        ),
        other => panic!("{} is not a forward: {other:?}", f.name),
    }
}

const TS: &str = r#"import { Foo } from "./foo";
/** Text area wrapper. */
export class Doc implements Editor {
  private items: Map<string, number> = new Map();
  /** Insert text at an offset. */
  public insert(text: string, offset: number): void { this.area.insert(text, offset); }
  getItems(): Map<string, number> { return this.items; }
}
export function pair(a: number): [number, boolean] {
  const result = a + 1; // add one to a
  try { risky(a); } catch (e) {}
  return [result, true];
}
function hidden() { log("x", 3); }
"#;

#[test]
fn typescript() {
    let file = facts("src/doc.ts", TS);
    let insert = function(&file, "insert");
    assert_eq!(
        forward_of(insert),
        (
            "this.area".into(),
            "insert".into(),
            vec!["text".into(), "offset".into()]
        )
    );
    assert!(insert.public && insert.overrides && !insert.returns_value);
    assert_eq!(insert.params, ["text", "offset"]);
    let doc = &file.comments[insert.doc.expect("insert has a doc comment")];
    assert_eq!(doc.summary(), "Insert text at an offset.");
    assert!(doc.code.contains("insert(text: string, offset: number)"));
    assert_eq!(
        function(&file, "getItems").body,
        Body::ReturnsField("items".into())
    );
    let pair = function(&file, "pair");
    assert!(pair.public && pair.returns_value && pair.returns_tuple && !pair.overrides);
    assert!(!function(&file, "hidden").public);
    let items = file
        .fields
        .iter()
        .find(|x| x.name == "items")
        .expect("field");
    assert!(items.collection && !items.public);
    assert_eq!(file.types[0].name, "Doc");
    assert!(file.types[0].doc.is_some());
    let trailing = file
        .comments
        .iter()
        .find(|c| c.text == "add one to a")
        .expect("trailing comment");
    assert_eq!(trailing.code, "const result = a + 1;");
    assert_eq!(file.ignored_errors.len(), 1);
    assert_eq!(file.ignored_errors[0].callee.as_deref(), Some("risky"));
    let log = file.calls.iter().find(|c| c.callee == "log").expect("call");
    assert!(log.args.iter().all(|a| a.literal));
    assert_eq!(file.imports.len(), 1);
    assert!(
        file.locals
            .iter()
            .any(|l| l.name == "result" && l.declared == 10 && l.last_use == 12)
    );
}

const PY: &str = r#"from foo import bar
class Doc(Editor):
    """Text area wrapper."""
    def __init__(self):
        self._items = {}
    def insert(self, text, offset):
        """Insert text at an offset."""
        return self.area.insert(text, offset)
    def get_items(self):
        return self._items
def pair(a) -> tuple[int, bool]:
    result = a + 1  # add one to a
    try:
        risky(a)
    except ValueError:
        pass
    return result, True
def _hidden():
    log("x", 3)
"#;

#[test]
fn python() {
    let file = facts("doc.py", PY);
    let insert = function(&file, "insert");
    assert_eq!(
        forward_of(insert),
        (
            "self.area".into(),
            "insert".into(),
            vec!["text".into(), "offset".into()]
        )
    );
    assert!(insert.public && insert.overrides && insert.returns_value);
    assert_eq!(insert.statements, 1);
    assert_eq!(
        file.comments[insert.doc.expect("docstring")].summary(),
        "Insert text at an offset."
    );
    assert_eq!(
        function(&file, "get_items").body,
        Body::ReturnsField("_items".into())
    );
    assert!(function(&file, "pair").returns_tuple);
    assert!(!function(&file, "_hidden").public);
    let items = file
        .fields
        .iter()
        .find(|x| x.name == "_items")
        .expect("field");
    assert!(items.collection && !items.public);
    assert!(file.types[0].doc.is_some());
    assert_eq!(file.ignored_errors.len(), 1);
    assert_eq!(file.ignored_errors[0].callee.as_deref(), Some("risky"));
    assert!(
        file.comments
            .iter()
            .any(|c| c.text == "add one to a" && c.code == "result = a + 1")
    );
}

const PHP: &str = r"<?php
use App\Foo;
/** Text area wrapper. */
class Doc implements Editor {
    private array $items = [];
    /** Insert text at an offset. */
    public function insert(string $text, int $offset): void { $this->area->insert($text, $offset); }
    public function getItems(): array { return $this->items; }
}
function pair($a) {
    $result = $a + 1;
    try { risky($a); } catch (\Exception $e) {}
    return $result;
}
";

#[test]
fn php() {
    let file = facts("Doc.php", PHP);
    let insert = function(&file, "insert");
    assert_eq!(
        forward_of(insert),
        (
            "$this->area".into(),
            "insert".into(),
            vec!["text".into(), "offset".into()]
        )
    );
    assert!(insert.public && insert.overrides && !insert.returns_value);
    assert!(insert.doc.is_some());
    assert_eq!(
        function(&file, "getItems").body,
        Body::ReturnsField("items".into())
    );
    let items = file
        .fields
        .iter()
        .find(|x| x.name == "items")
        .expect("field");
    assert!(items.collection && !items.public);
    assert_eq!(file.ignored_errors[0].callee.as_deref(), Some("risky"));
    assert!(function(&file, "pair").returns_value);
    assert!(file.locals.iter().any(|l| l.name == "result"));
}

const RS: &str = r"use crate::foo::Foo;
/// Text area wrapper.
pub struct Doc { items: Vec<u32>, pub name: String }
impl Doc {
    /// Insert text at an offset.
    pub fn insert(&mut self, text: &str, offset: usize) { self.area.insert(text, offset) }
    pub fn items(&self) -> &Vec<u32> { &self.items }
}
impl Editor for Doc {
    fn pair(&self) -> (u32, bool) {
        let result = 1;
        let _ = risky(result);
        helper(1).ok();
        (result, true)
    }
}
";

#[test]
fn rust() {
    let file = facts("src/doc.rs", RS);
    let insert = function(&file, "insert");
    assert_eq!(
        forward_of(insert),
        (
            "self.area".into(),
            "insert".into(),
            vec!["text".into(), "offset".into()]
        )
    );
    assert!(insert.public && !insert.overrides && !insert.returns_value);
    assert_eq!(
        file.comments[insert.doc.expect("doc")].summary(),
        "Insert text at an offset."
    );
    assert_eq!(
        function(&file, "items").body,
        Body::ReturnsField("items".into())
    );
    let pair = function(&file, "pair");
    assert!(pair.overrides && pair.returns_value && pair.returns_tuple);
    let items = file
        .fields
        .iter()
        .find(|x| x.name == "items")
        .expect("field");
    assert!(items.collection && !items.public && items.owner == "Doc");
    assert!(file.types[0].doc.is_some());
    let ignored: Vec<_> = file
        .ignored_errors
        .iter()
        .map(|e| e.callee.as_deref())
        .collect();
    assert_eq!(ignored, [Some("risky"), Some("helper")]);
}

#[test]
fn directive_comments_between_doc_and_declaration_keep_the_doc_attached() {
    let source = "/** Sums two numbers. */\n// eslint-disable-next-line no-magic-numbers\nexport function add(a: number, b: number) { return a + b; }\n";
    let file = facts("add.ts", source);
    let doc = function(&file, "add").doc.expect("doc stays attached");
    assert_eq!(file.comments[doc].summary(), "Sums two numbers.");
}

#[test]
fn javascript_parses_with_the_tsx_grammar() {
    let file = facts(
        "app.js",
        "export function show(el) {\n  return <div>{render(el)}</div>;\n}\n",
    );
    assert!(function(&file, "show").returns_value);
    assert!(file.calls.iter().any(|c| c.callee == "render"));
}

/// A component used as JSX is called: each prop is a named argument.
#[test]
fn jsx_elements_call_their_component() {
    let file = facts(
        "app.tsx",
        "export function App(p) {\n  return <div><Ui.Card title=\"Hi\" size={2} disabled {...p}><Row /></Ui.Card></div>;\n}\n",
    );
    let args = |callee: &str| -> Vec<(Option<String>, String, bool)> {
        let call = file
            .calls
            .iter()
            .find(|c| c.callee == callee)
            .unwrap_or_else(|| panic!("no call to {callee}"));
        call.args
            .iter()
            .map(|a| (a.name.clone(), a.text.clone(), a.literal))
            .collect()
    };
    assert_eq!(
        args("Card"),
        [
            (Some("title".into()), "\"Hi\"".into(), true),
            (Some("size".into()), "2".into(), true),
            (Some("disabled".into()), "true".into(), true),
            (None, "{...p}".into(), false),
        ]
    );
    assert_eq!(
        file.calls
            .iter()
            .find(|c| c.callee == "Card")
            .map(|c| c.receiver.as_str()),
        Some("Ui")
    );
    assert_eq!(args("Row"), []);
    assert!(
        !file.calls.iter().any(|c| c.callee == "div"),
        "DOM elements are not calls"
    );
}

/// `forwardRef` and `memo` components are named by their `const`; other wrappers are not.
#[test]
fn react_wrappers_name_their_component() {
    let file = facts(
        "ui.tsx",
        "export const Fancy = React.forwardRef((props, ref) => <b ref={ref} {...props} />);\n\
         export const Quiet = memo(forwardRef(function (props, ref) { return <Icons.spinner />; }));\n\
         export const save = debounce(() => store());\n",
    );
    assert_eq!(function(&file, "Fancy").params, ["props", "ref"]);
    assert_eq!(function(&file, "Quiet").params, ["props", "ref"]);
    assert!(
        !file.functions.iter().any(|f| f.name == "save"),
        "only React's wrappers name a component"
    );
    assert!(
        file.calls
            .iter()
            .any(|c| c.callee == "spinner" && c.receiver == "Icons"),
        "a member tag is a component, whatever its case"
    );
}

#[test]
fn vue_lines_match_the_component_file() {
    let source = "<template>\n  <div/>\n</template>\n<script setup lang=\"ts\">\nfunction go(a: number) { return run(a); }\n</script>\n";
    let file = facts("Comp.vue", source);
    assert_eq!(function(&file, "go").span.start, 5);
}

#[test]
fn copies_that_differ_only_in_names_hash_the_same() {
    let source = "function a(x) {\n  if (x) {\n    send(x, 1);\n    log(x);\n  }\n}\nfunction b(y) {\n  if (y) {\n    send(y, 2);\n    log(y);\n  }\n}\n";
    let file = facts("dup.ts", source);
    let ifs: Vec<_> = file
        .fragments
        .iter()
        .filter(|g| g.span.start == 2 || g.span.start == 8)
        .collect();
    assert_eq!(ifs.len(), 2);
    assert_eq!(ifs[0].hash, ifs[1].hash);
}

#[test]
fn unsupported_and_generated_files_are_skipped() {
    assert!(parse("README.md", "# hi").is_none());
    assert!(parse("types.d.ts", "declare const x: number;").is_none());
    assert!(parse("bundle.min.js", "a()").is_none());
}
