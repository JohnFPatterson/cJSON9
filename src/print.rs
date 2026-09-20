#![allow(dead_code)]
//! Safe JSON printer matching cJSON 1.7.19 byte-for-byte formatting.

use crate::traits::{PrintBuf, TreeReader};
use crate::types::{
    type_mask, CJSON_ARRAY, CJSON_FALSE, CJSON_NESTING_LIMIT, CJSON_NULL, CJSON_NUMBER,
    CJSON_OBJECT, CJSON_RAW, CJSON_STRING, CJSON_TRUE,
};

/// Render `item` into `buf`. Returns false on allocation/nesting failure.
///
/// Formatted objects: `{\n`, tab indent per depth, `:\t` after keys, `,\n` between
/// members. Formatted arrays: comma+space, no extra newlines for a flat array.
/// Unformatted: no spaces. Invalid type bits fail. Raw prints `valuestring` as-is.
pub fn print_value<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where
    R: TreeReader,
    P: PrintBuf,
{
    match type_mask(reader.raw_type(item)) {
        CJSON_NULL => buf.write(b"null"),
        CJSON_FALSE => buf.write(b"false"),
        CJSON_TRUE => buf.write(b"true"),
        CJSON_NUMBER => print_number(reader, item, buf),
        CJSON_RAW => match reader.valuestring(item) {
            Some(raw) => buf.write(raw),
            None => false,
        },
        CJSON_STRING => print_string_ptr(reader.valuestring(item), buf),
        CJSON_ARRAY => print_array(reader, item, buf),
        CJSON_OBJECT => print_object(reader, item, buf),
        _ => false,
    }
}

/// Escape `input` as a JSON string (including surrounding quotes) into `buf`.
/// NULL/empty input must print `""`.
pub fn print_string_ptr<P: PrintBuf>(input: Option<&[u8]>, buf: &mut P) -> bool {
    let input = match input {
        Some(s) if !s.is_empty() => s,
        _ => return buf.write(b"\"\""),
    };

    if !buf.write(b"\"") {
        return false;
    }

    let mut i = 0;
    while i < input.len() {
        let start = i;
        while i < input.len() {
            let c = input[i];
            if c > 31 && c != b'"' && c != b'\\' {
                i += 1;
            } else {
                break;
            }
        }
        if start < i && !buf.write(&input[start..i]) {
            return false;
        }
        if i < input.len() {
            if !write_escaped(buf, input[i]) {
                return false;
            }
            i += 1;
        }
    }

    buf.write(b"\"")
}

fn print_number<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where
    R: TreeReader,
    P: PrintBuf,
{
    let valuedouble = reader.valuedouble(item);
    let valueint = reader.valueint(item);
    let mut stack_buf = [0u8; 26];
    let n = match buf
        .number_printer()
        .print_number(valuedouble, valueint, &mut stack_buf)
    {
        Some(n) if n <= stack_buf.len() => n,
        _ => return false,
    };
    buf.write(&stack_buf[..n])
}

fn print_array<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where
    R: TreeReader,
    P: PrintBuf,
{
    if buf.depth() >= CJSON_NESTING_LIMIT {
        return false;
    }
    if !buf.write(b"[") {
        return false;
    }
    buf.inc_depth();

    let mut current = reader.child(item);
    while let Some(elem) = current {
        if !print_value(reader, elem, buf) {
            return false;
        }
        current = reader.next(elem);
        if current.is_some() {
            let sep: &[u8] = if buf.formatted() { b", " } else { b"," };
            if !buf.write(sep) {
                return false;
            }
        }
    }

    if !buf.write(b"]") {
        return false;
    }
    buf.dec_depth();
    true
}

fn print_object<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where
    R: TreeReader,
    P: PrintBuf,
{
    if buf.depth() >= CJSON_NESTING_LIMIT {
        return false;
    }
    if buf.formatted() {
        if !buf.write(b"{\n") {
            return false;
        }
    } else if !buf.write(b"{") {
        return false;
    }
    buf.inc_depth();

    let mut current = reader.child(item);
    while let Some(elem) = current {
        if buf.formatted() {
            let depth = buf.depth();
            if !write_tabs(buf, depth) {
                return false;
            }
        }
        if !print_string_ptr(reader.key(elem), buf) {
            return false;
        }
        if buf.formatted() {
            if !buf.write(b":\t") {
                return false;
            }
        } else if !buf.write(b":") {
            return false;
        }
        if !print_value(reader, elem, buf) {
            return false;
        }
        current = reader.next(elem);
        if current.is_some() && !buf.write(b",") {
            return false;
        }
        if buf.formatted() && !buf.write(b"\n") {
            return false;
        }
    }

    if buf.formatted() {
        let n = buf.depth().saturating_sub(1);
        if !write_tabs(buf, n) {
            return false;
        }
    }
    if !buf.write(b"}") {
        return false;
    }
    buf.dec_depth();
    true
}

fn write_escaped<P: PrintBuf>(buf: &mut P, c: u8) -> bool {
    match c {
        b'"' => buf.write(b"\\\""),
        b'\\' => buf.write(b"\\\\"),
        0x08 => buf.write(b"\\b"),
        0x0c => buf.write(b"\\f"),
        b'\n' => buf.write(b"\\n"),
        b'\r' => buf.write(b"\\r"),
        b'\t' => buf.write(b"\\t"),
        _ => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let seq = [
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[(c >> 4) as usize],
                HEX[(c & 0x0f) as usize],
            ];
            buf.write(&seq)
        }
    }
}

fn write_tabs<P: PrintBuf>(buf: &mut P, n: usize) -> bool {
    const TABS: [u8; 32] = [b'\t'; 32];
    let mut left = n;
    while left > 0 {
        let chunk = left.min(TABS.len());
        if !buf.write(&TABS[..chunk]) {
            return false;
        }
        left -= chunk;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{NumberPrinter, PrintBuf, TreeReader};
    use crate::types::{CJSON_INVALID, CJSON_IS_REFERENCE};

    struct TestNumbers;

    impl NumberPrinter for TestNumbers {
        fn print_number(&self, valuedouble: f64, valueint: i32, buf: &mut [u8]) -> Option<usize> {
            if buf.len() < 26 {
                return None;
            }
            if !valuedouble.is_finite() {
                buf[..4].copy_from_slice(b"null");
                return Some(4);
            }
            let s = if valuedouble == f64::from(valueint) {
                valueint.to_string()
            } else {
                format!("{}", valuedouble)
            };
            if s.len() > buf.len() {
                return None;
            }
            buf[..s.len()].copy_from_slice(s.as_bytes());
            Some(s.len())
        }
    }

    struct FailNumbers;

    impl NumberPrinter for FailNumbers {
        fn print_number(&self, _valuedouble: f64, _valueint: i32, _buf: &mut [u8]) -> Option<usize> {
            None
        }
    }

    struct TestBuf<N> {
        out: Vec<u8>,
        format: bool,
        depth: usize,
        numbers: N,
        fail: bool,
    }

    impl TestBuf<TestNumbers> {
        fn new(format: bool) -> Self {
            Self {
                out: Vec::new(),
                format,
                depth: 0,
                numbers: TestNumbers,
                fail: false,
            }
        }
    }

    impl TestBuf<FailNumbers> {
        fn failing_numbers() -> Self {
            Self {
                out: Vec::new(),
                format: false,
                depth: 0,
                numbers: FailNumbers,
                fail: false,
            }
        }
    }

    impl<N> TestBuf<N> {
        fn as_str(&self) -> &str {
            std::str::from_utf8(&self.out).expect("utf-8 output")
        }
    }

    impl<N: NumberPrinter> PrintBuf for TestBuf<N> {
        fn formatted(&self) -> bool {
            self.format
        }

        fn depth(&self) -> usize {
            self.depth
        }

        fn inc_depth(&mut self) {
            self.depth += 1;
        }

        fn dec_depth(&mut self) {
            self.depth = self.depth.saturating_sub(1);
        }

        fn write(&mut self, bytes: &[u8]) -> bool {
            if self.fail {
                return false;
            }
            self.out.extend_from_slice(bytes);
            true
        }

        fn number_printer(&self) -> &dyn NumberPrinter {
            &self.numbers
        }
    }

    #[derive(Default)]
    struct Node {
        ty: i32,
        valuedouble: f64,
        valueint: i32,
        valuestring: Option<Vec<u8>>,
        key: Option<Vec<u8>>,
        child: Option<usize>,
        next: Option<usize>,
    }

    struct Tree {
        nodes: Vec<Node>,
    }

    impl Tree {
        fn new() -> Self {
            Self { nodes: Vec::new() }
        }

        fn alloc(&mut self, node: Node) -> usize {
            let id = self.nodes.len();
            self.nodes.push(node);
            id
        }

        fn link(&mut self, parent: usize, children: &[usize]) {
            if children.is_empty() {
                return;
            }
            self.nodes[parent].child = Some(children[0]);
            for pair in children.windows(2) {
                self.nodes[pair[0]].next = Some(pair[1]);
            }
        }

        fn null(&mut self) -> usize {
            self.alloc(Node {
                ty: CJSON_NULL,
                ..Node::default()
            })
        }

        fn bool(&mut self, value: bool) -> usize {
            self.alloc(Node {
                ty: if value { CJSON_TRUE } else { CJSON_FALSE },
                ..Node::default()
            })
        }

        fn number(&mut self, n: i32) -> usize {
            self.alloc(Node {
                ty: CJSON_NUMBER,
                valuedouble: f64::from(n),
                valueint: n,
                ..Node::default()
            })
        }

        fn number_raw(&mut self, valuedouble: f64, valueint: i32) -> usize {
            self.alloc(Node {
                ty: CJSON_NUMBER,
                valuedouble,
                valueint,
                ..Node::default()
            })
        }

        fn string(&mut self, s: &[u8]) -> usize {
            self.alloc(Node {
                ty: CJSON_STRING,
                valuestring: Some(s.to_vec()),
                ..Node::default()
            })
        }

        fn string_none(&mut self) -> usize {
            self.alloc(Node {
                ty: CJSON_STRING,
                valuestring: None,
                ..Node::default()
            })
        }

        fn raw(&mut self, s: Option<&[u8]>) -> usize {
            self.alloc(Node {
                ty: CJSON_RAW,
                valuestring: s.map(|b| b.to_vec()),
                ..Node::default()
            })
        }

        fn array(&mut self, elems: &[usize]) -> usize {
            let id = self.alloc(Node {
                ty: CJSON_ARRAY,
                ..Node::default()
            });
            self.link(id, elems);
            id
        }

        fn object(&mut self, members: &[(&[u8], usize)]) -> usize {
            let id = self.alloc(Node {
                ty: CJSON_OBJECT,
                ..Node::default()
            });
            let mut kids = Vec::with_capacity(members.len());
            for &(key, child) in members {
                self.nodes[child].key = Some(key.to_vec());
                kids.push(child);
            }
            self.link(id, &kids);
            id
        }

        fn invalid(&mut self) -> usize {
            self.alloc(Node {
                ty: CJSON_INVALID,
                ..Node::default()
            })
        }

        fn with_type(&mut self, ty: i32) -> usize {
            self.alloc(Node {
                ty,
                ..Node::default()
            })
        }
    }

    impl TreeReader for Tree {
        type Handle = usize;

        fn raw_type(&self, item: Self::Handle) -> i32 {
            self.nodes[item].ty
        }

        fn valuedouble(&self, item: Self::Handle) -> f64 {
            self.nodes[item].valuedouble
        }

        fn valueint(&self, item: Self::Handle) -> i32 {
            self.nodes[item].valueint
        }

        fn valuestring(&self, item: Self::Handle) -> Option<&[u8]> {
            self.nodes[item].valuestring.as_deref()
        }

        fn key(&self, item: Self::Handle) -> Option<&[u8]> {
            self.nodes[item].key.as_deref()
        }

        fn child(&self, item: Self::Handle) -> Option<Self::Handle> {
            self.nodes[item].child
        }

        fn next(&self, item: Self::Handle) -> Option<Self::Handle> {
            self.nodes[item].next
        }
    }

    fn render(tree: &Tree, item: usize, format: bool) -> Option<String> {
        let mut buf = TestBuf::new(format);
        if print_value(tree, item, &mut buf) {
            Some(buf.as_str().to_string())
        } else {
            None
        }
    }

    fn render_string(input: Option<&[u8]>) -> Option<String> {
        let mut buf = TestBuf::new(false);
        if print_string_ptr(input, &mut buf) {
            Some(buf.as_str().to_string())
        } else {
            None
        }
    }

    #[test]
    fn print_string_empty_and_none() {
        assert_eq!(render_string(None).unwrap(), "\"\"");
        assert_eq!(render_string(Some(b"")).unwrap(), "\"\"");
    }

    #[test]
    fn print_string_ascii_table() {
        let ascii: Vec<u8> = (1..0x7F).collect();
        let expected = "\"\\u0001\\u0002\\u0003\\u0004\\u0005\\u0006\\u0007\\b\\t\\n\\u000b\\f\\r\\u000e\\u000f\\u0010\\u0011\\u0012\\u0013\\u0014\\u0015\\u0016\\u0017\\u0018\\u0019\\u001a\\u001b\\u001c\\u001d\\u001e\\u001f !\\\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\"";
        assert_eq!(render_string(Some(&ascii)).unwrap(), expected);
    }

    #[test]
    fn print_string_utf8_passthrough() {
        assert_eq!(render_string(Some("ü猫慕".as_bytes())).unwrap(), "\"ü猫慕\"");
    }

    #[test]
    fn print_value_literals() {
        let mut t = Tree::new();
        let n = t.null();
        let tr = t.bool(true);
        let fa = t.bool(false);
        assert_eq!(render(&t, n, false).unwrap(), "null");
        assert_eq!(render(&t, tr, false).unwrap(), "true");
        assert_eq!(render(&t, fa, false).unwrap(), "false");
    }

    #[test]
    fn print_value_number_integers_and_reals() {
        let mut t = Tree::new();
        let zero = t.number(0);
        let neg = t.number(-1);
        let min = t.number(i32::MIN);
        let max = t.number(i32::MAX);
        let real = t.number_raw(1.5, 1);
        let nan = t.number_raw(f64::NAN, 0);
        let inf = t.number_raw(f64::INFINITY, i32::MAX);
        let ninf = t.number_raw(f64::NEG_INFINITY, i32::MIN);
        assert_eq!(render(&t, zero, false).unwrap(), "0");
        assert_eq!(render(&t, neg, false).unwrap(), "-1");
        assert_eq!(render(&t, min, false).unwrap(), "-2147483648");
        assert_eq!(render(&t, max, false).unwrap(), "2147483647");
        assert_eq!(render(&t, real, false).unwrap(), "1.5");
        assert_eq!(render(&t, nan, false).unwrap(), "null");
        assert_eq!(render(&t, inf, false).unwrap(), "null");
        assert_eq!(render(&t, ninf, false).unwrap(), "null");
    }

    #[test]
    fn print_value_string_and_raw() {
        let mut t = Tree::new();
        let empty = t.string(b"");
        let hello = t.string(b"hello");
        let none_s = t.string_none();
        let raw = t.raw(Some(b"[1,2,3]"));
        let raw_none = t.raw(None);
        assert_eq!(render(&t, empty, false).unwrap(), "\"\"");
        assert_eq!(render(&t, hello, false).unwrap(), "\"hello\"");
        assert_eq!(render(&t, none_s, false).unwrap(), "\"\"");
        assert_eq!(render(&t, raw, false).unwrap(), "[1,2,3]");
        assert!(render(&t, raw_none, false).is_none());
    }

    #[test]
    fn print_array_goldens() {
        let mut t = Tree::new();
        let empty = t.array(&[]);
        assert_eq!(render(&t, empty, false).unwrap(), "[]");
        assert_eq!(render(&t, empty, true).unwrap(), "[]");

        let one = t.number(1);
        let a1 = t.array(&[one]);
        assert_eq!(render(&t, a1, false).unwrap(), "[1]");
        assert_eq!(render(&t, a1, true).unwrap(), "[1]");

        let hello = t.string(b"hello!");
        let a_str = t.array(&[hello]);
        assert_eq!(render(&t, a_str, false).unwrap(), "[\"hello!\"]");

        let inner = t.array(&[]);
        let nested = t.array(&[inner]);
        assert_eq!(render(&t, nested, false).unwrap(), "[[]]");
        assert_eq!(render(&t, nested, true).unwrap(), "[[]]");

        let n = t.null();
        let a_null = t.array(&[n]);
        assert_eq!(render(&t, a_null, false).unwrap(), "[null]");

        let n1 = t.number(1);
        let n2 = t.number(2);
        let n3 = t.number(3);
        let multi = t.array(&[n1, n2, n3]);
        assert_eq!(render(&t, multi, false).unwrap(), "[1,2,3]");
        assert_eq!(render(&t, multi, true).unwrap(), "[1, 2, 3]");

        let v1 = t.number(1);
        let vnull = t.null();
        let vtrue = t.bool(true);
        let vfalse = t.bool(false);
        let varr = t.array(&[]);
        let vhello = t.string(b"hello");
        let vobj = t.object(&[]);
        let mixed = t.array(&[v1, vnull, vtrue, vfalse, varr, vhello, vobj]);
        assert_eq!(
            render(&t, mixed, false).unwrap(),
            "[1,null,true,false,[],\"hello\",{}]"
        );
        assert_eq!(
            render(&t, mixed, true).unwrap(),
            "[1, null, true, false, [], \"hello\", {\n\t}]"
        );
    }

    #[test]
    fn print_object_goldens() {
        let mut t = Tree::new();
        let empty = t.object(&[]);
        assert_eq!(render(&t, empty, false).unwrap(), "{}");
        assert_eq!(render(&t, empty, true).unwrap(), "{\n}");

        let one = t.number(1);
        let o1 = t.object(&[(b"one", one)]);
        assert_eq!(render(&t, o1, false).unwrap(), "{\"one\":1}");
        assert_eq!(render(&t, o1, true).unwrap(), "{\n\t\"one\":\t1\n}");

        let world = t.string(b"world!");
        let ohello = t.object(&[(b"hello", world)]);
        assert_eq!(render(&t, ohello, false).unwrap(), "{\"hello\":\"world!\"}");
        assert_eq!(
            render(&t, ohello, true).unwrap(),
            "{\n\t\"hello\":\t\"world!\"\n}"
        );

        let arr = t.array(&[]);
        let oarr = t.object(&[(b"array", arr)]);
        assert_eq!(render(&t, oarr, false).unwrap(), "{\"array\":[]}");
        assert_eq!(render(&t, oarr, true).unwrap(), "{\n\t\"array\":\t[]\n}");

        let n = t.null();
        let onull = t.object(&[(b"null", n)]);
        assert_eq!(render(&t, onull, false).unwrap(), "{\"null\":null}");
        assert_eq!(render(&t, onull, true).unwrap(), "{\n\t\"null\":\tnull\n}");

        let a = t.number(1);
        let b = t.number(2);
        let c = t.number(3);
        let multi = t.object(&[(b"one", a), (b"two", b), (b"three", c)]);
        assert_eq!(
            render(&t, multi, false).unwrap(),
            "{\"one\":1,\"two\":2,\"three\":3}"
        );
        assert_eq!(
            render(&t, multi, true).unwrap(),
            "{\n\t\"one\":\t1,\n\t\"two\":\t2,\n\t\"three\":\t3\n}"
        );

        let one = t.number(1);
        let nul = t.null();
        let tru = t.bool(true);
        let fal = t.bool(false);
        let arr = t.array(&[]);
        let hello = t.string(b"hello");
        let obj = t.object(&[]);
        let mixed = t.object(&[
            (b"one", one),
            (b"NULL", nul),
            (b"TRUE", tru),
            (b"FALSE", fal),
            (b"array", arr),
            (b"world", hello),
            (b"object", obj),
        ]);
        assert_eq!(
            render(&t, mixed, false).unwrap(),
            "{\"one\":1,\"NULL\":null,\"TRUE\":true,\"FALSE\":false,\"array\":[],\"world\":\"hello\",\"object\":{}}"
        );
        assert_eq!(
            render(&t, mixed, true).unwrap(),
            "{\n\t\"one\":\t1,\n\t\"NULL\":\tnull,\n\t\"TRUE\":\ttrue,\n\t\"FALSE\":\tfalse,\n\t\"array\":\t[],\n\t\"world\":\t\"hello\",\n\t\"object\":\t{\n\t}\n}"
        );
    }

    #[test]
    fn print_readme_formatted_monitor() {
        let mut t = Tree::new();
        let w0 = t.number(1280);
        let h0 = t.number(720);
        let r0 = t.object(&[(b"width", w0), (b"height", h0)]);
        let w1 = t.number(1920);
        let h1 = t.number(1080);
        let r1 = t.object(&[(b"width", w1), (b"height", h1)]);
        let w2 = t.number(3840);
        let h2 = t.number(2160);
        let r2 = t.object(&[(b"width", w2), (b"height", h2)]);
        let resolutions = t.array(&[r0, r1, r2]);
        let name = t.string(b"Awesome 4K");
        let monitor = t.object(&[(b"name", name), (b"resolutions", resolutions)]);

        let expected = "{\n\
\t\"name\":\t\"Awesome 4K\",\n\
\t\"resolutions\":\t[{\n\
\t\t\t\"width\":\t1280,\n\
\t\t\t\"height\":\t720\n\
\t\t}, {\n\
\t\t\t\"width\":\t1920,\n\
\t\t\t\"height\":\t1080\n\
\t\t}, {\n\
\t\t\t\"width\":\t3840,\n\
\t\t\t\"height\":\t2160\n\
\t\t}]\n\
}";
        assert_eq!(render(&t, monitor, true).unwrap(), expected);
        assert_eq!(
            render(&t, monitor, false).unwrap(),
            "{\"name\":\"Awesome 4K\",\"resolutions\":[{\"width\":1280,\"height\":720},{\"width\":1920,\"height\":1080},{\"width\":3840,\"height\":2160}]}"
        );
    }

    #[test]
    fn print_masks_is_reference_bit() {
        let mut t = Tree::new();
        let item = t.with_type(CJSON_NULL | CJSON_IS_REFERENCE);
        assert_eq!(render(&t, item, false).unwrap(), "null");
    }

    #[test]
    fn print_invalid_type_fails() {
        let mut t = Tree::new();
        let item = t.invalid();
        assert!(render(&t, item, false).is_none());
    }

    #[test]
    fn print_number_printer_failure() {
        let mut t = Tree::new();
        let n = t.number(1);
        let mut buf = TestBuf::failing_numbers();
        assert!(!print_value(&t, n, &mut buf));
    }

    #[test]
    fn print_write_failure() {
        let mut t = Tree::new();
        let n = t.null();
        let mut buf = TestBuf::new(false);
        buf.fail = true;
        assert!(!print_value(&t, n, &mut buf));
        assert!(!print_string_ptr(Some(b"x"), &mut buf));
    }

    fn nest_arrays(count: usize) -> (Tree, usize) {
        let mut t = Tree::new();
        let mut inner = t.array(&[]);
        for _ in 1..count {
            inner = t.array(&[inner]);
        }
        (t, inner)
    }

    fn nest_objects(count: usize) -> (Tree, usize) {
        let mut t = Tree::new();
        let mut inner = t.object(&[]);
        for _ in 1..count {
            inner = t.object(&[(b"k", inner)]);
        }
        (t, inner)
    }

    #[test]
    fn print_nesting_limit_arrays() {
        let (t, root) = nest_arrays(CJSON_NESTING_LIMIT);
        assert!(render(&t, root, false).is_some());
        let (t, root) = nest_arrays(CJSON_NESTING_LIMIT + 1);
        assert!(render(&t, root, false).is_none());
    }

    #[test]
    fn print_nesting_limit_objects() {
        let (t, root) = nest_objects(CJSON_NESTING_LIMIT);
        assert!(render(&t, root, false).is_some());
        let (t, root) = nest_objects(CJSON_NESTING_LIMIT + 1);
        assert!(render(&t, root, false).is_none());
    }
}
