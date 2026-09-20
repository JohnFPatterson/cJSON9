#![allow(dead_code)]
//! `cJSON_Compare` matching cJSON 1.7.19.

use crate::traits::TreeReader;
use crate::types::{
    type_mask, CJSON_ARRAY, CJSON_FALSE, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT, CJSON_RAW,
    CJSON_STRING, CJSON_TRUE,
};

/// Relative epsilon compare used for numbers (`fabs(a-b) <= max(|a|,|b|) * DBL_EPSILON`).
pub fn compare_double(a: f64, b: f64) -> bool {
    let max_val = a.abs().max(b.abs());
    (a - b).abs() <= max_val * f64::EPSILON
}

/// Case-insensitive compare that treats two nulls as unequal (cJSON `case_insensitive_strcmp`).
///
/// C walks `unsigned char *` with `tolower` from ctype (C locale ≡ ASCII).
pub fn case_insensitive_strcmp(a: Option<&[u8]>, b: Option<&[u8]>) -> i32 {
    match (a, b) {
        (None, _) | (_, None) => 1,
        (Some(a), Some(b)) if core::ptr::eq(a, b) => 0,
        (Some(a), Some(b)) => {
            let mut i = 0;
            loop {
                let ca = a.get(i).copied().unwrap_or(0).to_ascii_lowercase();
                let cb = b.get(i).copied().unwrap_or(0).to_ascii_lowercase();
                if ca != cb {
                    return i32::from(ca) - i32::from(cb);
                }
                let raw_a = a.get(i).copied().unwrap_or(0);
                if raw_a == 0 {
                    return 0;
                }
                i += 1;
            }
        }
    }
}

/// Recursively compare two items. NULL or invalid types are unequal, even vs self.
/// `case_sensitive` applies to **object keys only**; string values are always sensitive.
pub fn compare<R: TreeReader>(
    reader: &R,
    a: Option<R::Handle>,
    b: Option<R::Handle>,
    case_sensitive: bool,
) -> bool
where
    R::Handle: PartialEq,
{
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        _ => return false,
    };

    let a_type = type_mask(reader.raw_type(a));
    let b_type = type_mask(reader.raw_type(b));
    if a_type != b_type {
        return false;
    }

    match a_type {
        CJSON_FALSE | CJSON_TRUE | CJSON_NULL | CJSON_NUMBER | CJSON_STRING | CJSON_RAW
        | CJSON_ARRAY | CJSON_OBJECT => {}
        _ => return false,
    }

    if a == b {
        return true;
    }

    match a_type {
        CJSON_FALSE | CJSON_TRUE | CJSON_NULL => true,
        CJSON_NUMBER => compare_double(reader.valuedouble(a), reader.valuedouble(b)),
        CJSON_STRING | CJSON_RAW => match (reader.valuestring(a), reader.valuestring(b)) {
            (Some(sa), Some(sb)) => sa == sb,
            _ => false,
        },
        CJSON_ARRAY => compare_array(reader, a, b, case_sensitive),
        CJSON_OBJECT => compare_object(reader, a, b, case_sensitive),
        _ => false,
    }
}

fn compare_array<R: TreeReader>(
    reader: &R,
    a: R::Handle,
    b: R::Handle,
    case_sensitive: bool,
) -> bool
where
    R::Handle: PartialEq,
{
    let mut a_element = reader.child(a);
    let mut b_element = reader.child(b);

    while let (Some(ae), Some(be)) = (a_element, b_element) {
        if !compare(reader, Some(ae), Some(be), case_sensitive) {
            return false;
        }
        a_element = reader.next(ae);
        b_element = reader.next(be);
    }

    // One array is longer than the other (C: `if (a_element != b_element)`).
    a_element.is_none() && b_element.is_none()
}

fn compare_object<R: TreeReader>(
    reader: &R,
    a: R::Handle,
    b: R::Handle,
    case_sensitive: bool,
) -> bool
where
    R::Handle: PartialEq,
{
    let mut a_element = reader.child(a);
    while let Some(ae) = a_element {
        let b_element = get_object_item(reader, b, reader.key(ae), case_sensitive);
        let Some(be) = b_element else {
            return false;
        };
        if !compare(reader, Some(ae), Some(be), case_sensitive) {
            return false;
        }
        a_element = reader.next(ae);
    }

    // Reverse walk so a subset of b is not equal.
    let mut b_element = reader.child(b);
    while let Some(be) = b_element {
        let a_element = get_object_item(reader, a, reader.key(be), case_sensitive);
        let Some(ae) = a_element else {
            return false;
        };
        if !compare(reader, Some(be), Some(ae), case_sensitive) {
            return false;
        }
        b_element = reader.next(be);
    }

    true
}

/// First-match object lookup matching C `get_object_item`.
///
/// Case-sensitive: a NULL key stops the search (while condition fails) and
/// the lookup returns None. Case-insensitive: a NULL key is unequal
/// (`case_insensitive_strcmp` returns 1) so the search continues.
fn get_object_item<R: TreeReader>(
    reader: &R,
    object: R::Handle,
    name: Option<&[u8]>,
    case_sensitive: bool,
) -> Option<R::Handle> {
    let name = name?;
    let mut current = reader.child(object);

    if case_sensitive {
        while let Some(elem) = current {
            match reader.key(elem) {
                Some(key) if key == name => break,
                Some(_) => current = reader.next(elem),
                None => break,
            }
        }
    } else {
        while let Some(elem) = current {
            if case_insensitive_strcmp(Some(name), reader.key(elem)) == 0 {
                break;
            }
            current = reader.next(elem);
        }
    }

    match current {
        Some(elem) if reader.key(elem).is_some() => Some(elem),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{case_insensitive_strcmp, compare, compare_double};
    use crate::traits::TreeReader;
    use crate::types::{
        CJSON_ARRAY, CJSON_FALSE, CJSON_INVALID, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT, CJSON_RAW,
        CJSON_STRING, CJSON_TRUE,
    };

    #[derive(Clone)]
    struct Node {
        ty: i32,
        valuedouble: f64,
        valuestring: Option<Vec<u8>>,
        key: Option<Vec<u8>>,
        child: Option<usize>,
        next: Option<usize>,
    }

    impl Node {
        fn typed(ty: i32) -> Self {
            Self {
                ty,
                valuedouble: 0.0,
                valuestring: None,
                key: None,
                child: None,
                next: None,
            }
        }
    }

    struct Tree {
        nodes: Vec<Node>,
    }

    impl Tree {
        fn new() -> Self {
            Self { nodes: Vec::new() }
        }

        fn add(&mut self, node: Node) -> usize {
            let id = self.nodes.len();
            self.nodes.push(node);
            id
        }

        fn null(&mut self) -> usize {
            self.add(Node::typed(CJSON_NULL))
        }

        fn boolean(&mut self, value: bool) -> usize {
            self.add(Node::typed(if value { CJSON_TRUE } else { CJSON_FALSE }))
        }

        fn number(&mut self, value: f64) -> usize {
            let mut n = Node::typed(CJSON_NUMBER);
            n.valuedouble = value;
            self.add(n)
        }

        fn string(&mut self, s: &str) -> usize {
            let mut n = Node::typed(CJSON_STRING);
            n.valuestring = Some(s.as_bytes().to_vec());
            self.add(n)
        }

        fn raw(&mut self, s: &str) -> usize {
            let mut n = Node::typed(CJSON_RAW);
            n.valuestring = Some(s.as_bytes().to_vec());
            self.add(n)
        }

        fn array(&mut self, children: &[usize]) -> usize {
            let id = self.add(Node::typed(CJSON_ARRAY));
            self.link(id, children);
            id
        }

        fn object(&mut self, members: &[(&str, usize)]) -> usize {
            let id = self.add(Node::typed(CJSON_OBJECT));
            let kids: Vec<usize> = members
                .iter()
                .map(|(k, child)| {
                    self.nodes[*child].key = Some(k.as_bytes().to_vec());
                    *child
                })
                .collect();
            self.link(id, &kids);
            id
        }

        fn link(&mut self, parent: usize, children: &[usize]) {
            self.nodes[parent].child = children.first().copied();
            for pair in children.windows(2) {
                self.nodes[pair[0]].next = Some(pair[1]);
            }
        }

        fn eq(&self, a: usize, b: usize, case_sensitive: bool) -> bool {
            compare(self, Some(a), Some(b), case_sensitive)
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
            0
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

    #[test]
    fn null_pointers_are_not_equal() {
        // tests/compare_tests.c: cjson_compare_should_compare_null_pointer_as_not_equal
        let t = Tree::new();
        assert!(!compare(&t, None, None, true));
        assert!(!compare(&t, None, None, false));
        let mut t = Tree::new();
        let n = t.null();
        assert!(!compare(&t, Some(n), None, true));
        assert!(!compare(&t, None, Some(n), false));
    }

    #[test]
    fn invalid_types_are_not_equal_even_vs_self() {
        // tests/compare_tests.c: cjson_compare_should_compare_invalid_as_not_equal
        // tests/compare_tests.c: cjson_compare_should_not_accept_invalid_types
        let mut t = Tree::new();
        let invalid = t.add(Node::typed(CJSON_INVALID));
        assert!(!compare(&t, Some(invalid), Some(invalid), false));
        assert!(!compare(&t, Some(invalid), Some(invalid), true));

        let mixed = t.add(Node::typed(CJSON_NUMBER | CJSON_STRING));
        assert!(!compare(&t, Some(mixed), Some(mixed), true));
        assert!(!compare(&t, Some(mixed), Some(mixed), false));
    }

    #[test]
    fn same_handle_is_equal_after_type_check() {
        let mut t = Tree::new();
        let n = t.number(1.0);
        assert!(compare(&t, Some(n), Some(n), true));
        let s = t.string("x");
        assert!(compare(&t, Some(s), Some(s), false));
    }

    #[test]
    fn compares_numbers() {
        // tests/compare_tests.c: cjson_compare_should_compare_numbers
        let mut t = Tree::new();
        let a = t.number(1.0);
        let b = t.number(1.0);
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));

        let a = t.number(0.0001);
        let b = t.number(0.0001);
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));

        let a = t.number(1e100);
        let b = t.number(10e99);
        assert!(t.eq(a, b, false));

        let a = t.number(0.5e-100);
        let b = t.number(0.5e-101);
        assert!(!t.eq(a, b, false));

        let a = t.number(1.0);
        let b = t.number(2.0);
        assert!(!t.eq(a, b, true));
        assert!(!t.eq(a, b, false));
    }

    #[test]
    fn compare_double_uses_relative_epsilon() {
        assert!(compare_double(1.0, 1.0));
        assert!(compare_double(0.0, 0.0));
        assert!(compare_double(1e100, 10e99));
        assert!(!compare_double(1.0, 2.0));
        let eps = f64::EPSILON;
        assert!(compare_double(1.0, 1.0 + eps));
        assert!(!compare_double(1.0, 1.0 + 3.0 * eps));
    }

    #[test]
    fn compares_booleans() {
        // tests/compare_tests.c: cjson_compare_should_compare_booleans
        let mut t = Tree::new();
        let tr = t.boolean(true);
        let tr2 = t.boolean(true);
        let fa = t.boolean(false);
        let fa2 = t.boolean(false);
        assert!(t.eq(tr, tr2, true));
        assert!(t.eq(tr, tr2, false));
        assert!(t.eq(fa, fa2, true));
        assert!(t.eq(fa, fa2, false));
        assert!(!t.eq(tr, fa, true));
        assert!(!t.eq(tr, fa, false));
        assert!(!t.eq(fa, tr, true));
        assert!(!t.eq(fa, tr, false));
    }

    #[test]
    fn compares_null() {
        // tests/compare_tests.c: cjson_compare_should_compare_null
        let mut t = Tree::new();
        let n1 = t.null();
        let n2 = t.null();
        let tr = t.boolean(true);
        assert!(t.eq(n1, n2, true));
        assert!(t.eq(n1, n2, false));
        assert!(!t.eq(n1, tr, true));
        assert!(!t.eq(n1, tr, false));
    }

    #[test]
    fn compares_strings_case_sensitively() {
        // tests/compare_tests.c: cjson_compare_should_compare_strings
        let mut t = Tree::new();
        let a = t.string("abcdefg");
        let b = t.string("abcdefg");
        let c = t.string("ABCDEFG");
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));
        assert!(!t.eq(c, a, true));
        assert!(!t.eq(c, a, false));
    }

    #[test]
    fn null_valuestring_is_not_equal() {
        let mut t = Tree::new();
        let a = t.add(Node::typed(CJSON_STRING));
        let b = t.string("x");
        assert!(!t.eq(a, b, true));
        let c = t.add(Node::typed(CJSON_STRING));
        // Distinct handles with NULL valuestring are unequal (C `strcmp` after NULL check).
        assert!(!t.eq(a, c, false));
        // Same handle with a valid type is equal before the valuestring check.
        assert!(compare(&t, Some(a), Some(a), true));
    }

    #[test]
    fn compares_raw() {
        // tests/compare_tests.c: cjson_compare_should_compare_raw
        let mut t = Tree::new();
        let a = t.raw("[true, false]");
        let b = t.raw("[true, false]");
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));
        let c = t.raw("[true, true]");
        assert!(!t.eq(a, c, true));
    }

    #[test]
    fn compares_arrays() {
        // tests/compare_tests.c: cjson_compare_should_compare_arrays
        let mut t = Tree::new();
        let empty_a = t.array(&[]);
        let empty_b = t.array(&[]);
        assert!(t.eq(empty_a, empty_b, true));
        assert!(t.eq(empty_a, empty_b, false));

        let mixed = |t: &mut Tree| {
            let f = t.boolean(false);
            let tr = t.boolean(true);
            let n = t.null();
            let num = t.number(42.0);
            let s = t.string("string");
            let arr = t.array(&[]);
            let obj = t.object(&[]);
            t.array(&[f, tr, n, num, s, arr, obj])
        };
        let a = mixed(&mut t);
        let b = mixed(&mut t);
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));

        let nested = |t: &mut Tree| {
            let one = t.number(1.0);
            let inner = t.array(&[one]);
            let two = t.number(2.0);
            let mid = t.array(&[inner, two]);
            t.array(&[mid])
        };
        let a = nested(&mut t);
        let b = nested(&mut t);
        assert!(t.eq(a, b, true));
        assert!(t.eq(a, b, false));

        let short = |t: &mut Tree| {
            let tr = t.boolean(true);
            let n = t.null();
            let num = t.number(42.0);
            let s = t.string("string");
            let arr = t.array(&[]);
            let obj = t.object(&[]);
            t.array(&[tr, n, num, s, arr, obj])
        };
        let a = short(&mut t);
        let b = mixed(&mut t);
        assert!(!t.eq(a, b, true));
        assert!(!t.eq(a, b, false));

        let prefix = |t: &mut Tree, vals: &[f64]| {
            let kids: Vec<usize> = vals.iter().map(|v| t.number(*v)).collect();
            t.array(&kids)
        };
        let a = prefix(&mut t, &[1.0, 2.0, 3.0]);
        let b = prefix(&mut t, &[1.0, 2.0]);
        assert!(!t.eq(a, b, true));
        assert!(!t.eq(a, b, false));
    }

    fn sample_object(t: &mut Tree, false_key: &str) -> usize {
        let f = t.boolean(false);
        let tr = t.boolean(true);
        let n = t.null();
        let num = t.number(42.0);
        let s = t.string("string");
        let arr = t.array(&[]);
        let obj = t.object(&[]);
        t.object(&[
            (false_key, f),
            ("true", tr),
            ("null", n),
            ("number", num),
            ("string", s),
            ("array", arr),
            ("object", obj),
        ])
    }

    fn sample_object_reordered(t: &mut Tree) -> usize {
        let tr = t.boolean(true);
        let f = t.boolean(false);
        let n = t.null();
        let num = t.number(42.0);
        let s = t.string("string");
        let arr = t.array(&[]);
        let obj = t.object(&[]);
        t.object(&[
            ("true", tr),
            ("false", f),
            ("null", n),
            ("number", num),
            ("string", s),
            ("array", arr),
            ("object", obj),
        ])
    }

    #[test]
    fn compares_objects() {
        // tests/compare_tests.c: cjson_compare_should_compare_objects
        let mut t = Tree::new();
        let empty_a = t.object(&[]);
        let empty_b = t.object(&[]);
        assert!(t.eq(empty_a, empty_b, true));
        assert!(t.eq(empty_a, empty_b, false));

        let a = sample_object(&mut t, "false");
        let b = sample_object_reordered(&mut t);
        assert!(t.eq(a, b, true));

        let a = sample_object(&mut t, "False");
        let b = sample_object_reordered(&mut t);
        assert!(!t.eq(a, b, true));
        assert!(t.eq(a, b, false));

        let a = sample_object(&mut t, "Flse");
        let b = sample_object_reordered(&mut t);
        assert!(!t.eq(a, b, false));

        let subset = |t: &mut Tree| {
            let one = t.number(1.0);
            let two = t.number(2.0);
            t.object(&[("one", one), ("two", two)])
        };
        let superset = |t: &mut Tree| {
            let one = t.number(1.0);
            let two = t.number(2.0);
            let three = t.number(3.0);
            t.object(&[("one", one), ("two", two), ("three", three)])
        };
        let a = subset(&mut t);
        let b = superset(&mut t);
        assert!(!t.eq(a, b, true));
        assert!(!t.eq(a, b, false));
    }

    #[test]
    fn object_key_lookup_is_first_match() {
        let mut t = Tree::new();
        let v1 = t.number(1.0);
        let v2 = t.number(2.0);
        let a = t.object(&[("x", v1)]);
        let b = t.object(&[("x", v2), ("x", v1)]);
        // a.x=1 looks up first x in b which is 2 → not equal
        assert!(!t.eq(a, b, true));

        let v1 = t.number(1.0);
        let v1b = t.number(1.0);
        let v2 = t.number(2.0);
        let a = t.object(&[("x", v1)]);
        let b = t.object(&[("x", v1b), ("x", v2)]);
        // a.x=1 matches b's first x=1; reverse: b's second x=2 looks up a's first x=1 → not equal
        assert!(!t.eq(a, b, true));
    }

    #[test]
    fn case_insensitive_strcmp_matches_c() {
        assert_eq!(case_insensitive_strcmp(None, None), 1);
        assert_eq!(case_insensitive_strcmp(None, Some(b"a")), 1);
        assert_eq!(case_insensitive_strcmp(Some(b"a"), None), 1);

        let s = b"Hello";
        assert_eq!(case_insensitive_strcmp(Some(s), Some(s)), 0);

        assert_eq!(case_insensitive_strcmp(Some(b"abc"), Some(b"ABC")), 0);
        assert_eq!(case_insensitive_strcmp(Some(b"abc"), Some(b"abc")), 0);
        assert_ne!(case_insensitive_strcmp(Some(b"abc"), Some(b"abd")), 0);
        assert!(case_insensitive_strcmp(Some(b"abc"), Some(b"abcd")) < 0);
        assert!(case_insensitive_strcmp(Some(b"abcd"), Some(b"abc")) > 0);
        assert_eq!(case_insensitive_strcmp(Some(b""), Some(b"")), 0);
    }
}
