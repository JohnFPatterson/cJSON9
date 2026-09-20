#![allow(dead_code)]
//! Safe JSON parser matching cJSON 1.7.19 quirks.
//!
//! Implement this module without `unsafe`. The shim supplies a [`TreeBuilder`].

use crate::traits::{ParseError, ParseOptions, TreeBuilder};
use crate::types::CJSON_NESTING_LIMIT;

/// Parse 4 hex digits (cJSON `parse_hex4`). Invalid digits yield 0, same as C.
pub fn parse_hex4(input: &[u8]) -> u32 {
    if input.len() < 4 {
        return 0;
    }
    let mut h: u32 = 0;
    for i in 0..4 {
        let c = input[i];
        h += match c {
            b'0'..=b'9' => u32::from(c - b'0'),
            b'A'..=b'F' => 10 + u32::from(c - b'A'),
            b'a'..=b'f' => 10 + u32::from(c - b'a'),
            _ => return 0,
        };
        if i < 3 {
            h <<= 4;
        }
    }
    h
}

/// Skip bytes `<= 32`. If this walks to `length`, step back one byte (cJSON quirk).
pub fn skip_whitespace(input: &[u8], offset: &mut usize) {
    if input.is_empty() {
        return;
    }
    while *offset < input.len() && input[*offset] <= 32 {
        *offset += 1;
    }
    if *offset == input.len() && !input.is_empty() {
        *offset -= 1;
    }
}

/// Skip UTF-8 BOM only when `offset == 0`. Requires `can_access_at_index(..., 4)`
/// which is `offset + 4 < length` (cJSON quirk: needs 5 bytes of buffer, not 3).
pub fn skip_utf8_bom(input: &[u8], offset: &mut usize) -> bool {
    if *offset != 0 {
        return false;
    }
    if offset.saturating_add(4) < input.len()
        && input.len() >= 3
        && input[0] == 0xEF
        && input[1] == 0xBB
        && input[2] == 0xBF
    {
        *offset += 3;
    }
    true
}

/// `can_read(buffer, size)`: `offset + size <= length`.
fn can_read(input: &[u8], offset: usize, size: usize) -> bool {
    offset
        .checked_add(size)
        .map(|end| end <= input.len())
        .unwrap_or(false)
}

/// `can_access_at_index(buffer, index)`: `offset + index < length`.
fn can_access_at_index(input: &[u8], offset: usize, index: usize) -> bool {
    offset
        .checked_add(index)
        .map(|i| i < input.len())
        .unwrap_or(false)
}

fn starts_with_at(input: &[u8], offset: usize, needle: &[u8]) -> bool {
    can_read(input, offset, needle.len()) && input[offset..offset + needle.len()] == *needle
}

/// Convert `\uXXXX` / `\uXXXX\uYYYY` at `seq` (starting at `\`) into UTF-8 at `out`.
/// Returns `(sequence_length, bytes_written)` or `None` on failure.
fn utf16_literal_to_utf8(seq: &[u8], out: &mut [u8]) -> Option<(usize, usize)> {
    if seq.len() < 6 {
        return None;
    }

    let first_code = parse_hex4(&seq[2..6]);
    if (0xDC00..=0xDFFF).contains(&first_code) {
        return None;
    }

    let (sequence_length, mut codepoint) = if (0xD800..=0xDBFF).contains(&first_code) {
        if seq.len() < 12 {
            return None;
        }
        if seq[6] != b'\\' || seq[7] != b'u' {
            return None;
        }
        let second_code = parse_hex4(&seq[8..12]);
        if !(0xDC00..=0xDFFF).contains(&second_code) {
            return None;
        }
        let cp = 0x10000 + (((first_code & 0x3FF) << 10) | (second_code & 0x3FF));
        (12usize, cp)
    } else {
        (6usize, first_code)
    };

    let (utf8_length, first_byte_mark) = if codepoint < 0x80 {
        (1usize, 0u32)
    } else if codepoint < 0x800 {
        (2, 0xC0)
    } else if codepoint < 0x10000 {
        (3, 0xE0)
    } else if codepoint <= 0x10FFFF {
        (4, 0xF0)
    } else {
        return None;
    };

    if out.len() < utf8_length {
        return None;
    }

    let mut utf8_position = utf8_length;
    while utf8_position > 1 {
        utf8_position -= 1;
        out[utf8_position] = ((codepoint | 0x80) & 0xBF) as u8;
        codepoint >>= 6;
    }
    if utf8_length > 1 {
        out[0] = ((codepoint | first_byte_mark) & 0xFF) as u8;
    } else {
        out[0] = (codepoint & 0x7F) as u8;
    }

    Some((sequence_length, utf8_length))
}

/// Unescape JSON string content `input[start..end]` (end is the closing quote).
/// On failure returns `Err(offset)` matching C's `input_pointer`.
fn unescape_into(input: &[u8], start: usize, end: usize, out: &mut [u8]) -> Result<usize, usize> {
    let mut src = start;
    let mut dst = 0usize;
    while src < end {
        if input[src] != b'\\' {
            if dst >= out.len() {
                return Err(src);
            }
            out[dst] = input[src];
            dst += 1;
            src += 1;
            continue;
        }

        // cJSON: `(input_end - input_pointer) < 1`
        if end.saturating_sub(src) < 1 {
            return Err(src);
        }
        if src + 1 >= input.len() {
            return Err(src);
        }

        let mut sequence_length = 2usize;
        match input[src + 1] {
            b'b' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = b'\x08';
                dst += 1;
            }
            b'f' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = b'\x0c';
                dst += 1;
            }
            b'n' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = b'\n';
                dst += 1;
            }
            b'r' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = b'\r';
                dst += 1;
            }
            b't' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = b'\t';
                dst += 1;
            }
            b'"' | b'\\' | b'/' => {
                if dst >= out.len() {
                    return Err(src);
                }
                out[dst] = input[src + 1];
                dst += 1;
            }
            b'u' => {
                let remaining = &input[src..end];
                let written = match utf16_literal_to_utf8(remaining, &mut out[dst..]) {
                    Some((seq, n)) => {
                        sequence_length = seq;
                        n
                    }
                    None => return Err(src),
                };
                dst += written;
            }
            _ => return Err(src),
        }
        src += sequence_length;
    }
    Ok(dst)
}

struct Parser<'a, B: TreeBuilder> {
    builder: &'a mut B,
    input: &'a [u8],
    offset: usize,
    depth: usize,
}

impl<'a, B: TreeBuilder> Parser<'a, B> {
    fn parse_value(&mut self, item: B::Handle) -> bool {
        if starts_with_at(self.input, self.offset, b"null") {
            self.builder.set_null(item);
            self.offset += 4;
            return true;
        }
        if starts_with_at(self.input, self.offset, b"false") {
            self.builder.set_bool(item, false);
            self.offset += 5;
            return true;
        }
        if starts_with_at(self.input, self.offset, b"true") {
            self.builder.set_bool(item, true);
            self.offset += 4;
            return true;
        }
        if can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b'"' {
            return self.parse_string(item);
        }
        if can_access_at_index(self.input, self.offset, 0) {
            let c = self.input[self.offset];
            if c == b'-' || c.is_ascii_digit() {
                return self.parse_number(item);
            }
        }
        if can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b'[' {
            return self.parse_array(item);
        }
        if can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b'{' {
            return self.parse_object(item);
        }
        false
    }

    fn parse_number(&mut self, item: B::Handle) -> bool {
        let start = self.offset;
        let mut len = 0usize;
        while can_access_at_index(self.input, start, len) {
            match self.input[start + len] {
                b'0'..=b'9' | b'+' | b'-' | b'e' | b'E' | b'.' => len += 1,
                _ => break,
            }
        }
        let token = &self.input[start..start + len];
        match self.builder.number_parser().parse_number(token) {
            Some(parsed) => {
                self.builder
                    .set_number(item, parsed.valuedouble, parsed.valueint);
                self.offset = start + parsed.consumed;
                true
            }
            None => false,
        }
    }

    fn parse_string(&mut self, item: B::Handle) -> bool {
        if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b'"' {
            // C still sets offset from `input_pointer = buffer_at_offset + 1`.
            self.offset = self.offset.saturating_add(1);
            return false;
        }

        let open = self.offset;
        let mut input_end = open + 1;
        let mut skipped_bytes = 0usize;
        while input_end < self.input.len() && self.input[input_end] != b'"' {
            if self.input[input_end] == b'\\' {
                if input_end + 1 >= self.input.len() {
                    self.offset = open + 1;
                    return false;
                }
                skipped_bytes += 1;
                input_end += 1;
            }
            input_end += 1;
        }
        if input_end >= self.input.len() || self.input[input_end] != b'"' {
            self.offset = open + 1;
            return false;
        }

        let cap = input_end.saturating_sub(open).saturating_sub(skipped_bytes);
        let input = self.input;
        let mut err_pos = open + 1;
        let ok = self.builder.set_string(item, cap, |out| {
            match unescape_into(input, open + 1, input_end, out) {
                Ok(n) => Some(n),
                Err(pos) => {
                    err_pos = pos;
                    None
                }
            }
        });
        if !ok {
            self.offset = err_pos;
            return false;
        }
        self.offset = input_end + 1;
        true
    }

    fn parse_array(&mut self, item: B::Handle) -> bool {
        if self.depth >= CJSON_NESTING_LIMIT {
            return false;
        }
        self.depth += 1;

        if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b'[' {
            return false;
        }

        self.offset += 1;
        skip_whitespace(self.input, &mut self.offset);
        if can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b']' {
            self.depth -= 1;
            self.builder.set_array(item);
            self.offset = self.offset.wrapping_add(1);
            return true;
        }

        if !can_access_at_index(self.input, self.offset, 0) {
            self.offset = self.offset.wrapping_sub(1);
            return false;
        }

        self.offset = self.offset.wrapping_sub(1);

        let mut head: Option<B::Handle> = None;
        loop {
            let new_item = match self.builder.new_item() {
                Some(h) => h,
                None => {
                    self.fail_child_chain(head);
                    return false;
                }
            };
            if !self.builder.append_child(item, new_item) {
                self.builder.delete(new_item);
                self.fail_child_chain(head);
                return false;
            }
            if head.is_none() {
                head = Some(new_item);
            }

            self.offset = self.offset.wrapping_add(1);
            skip_whitespace(self.input, &mut self.offset);
            if !self.parse_value(new_item) {
                self.fail_child_chain(head);
                return false;
            }
            skip_whitespace(self.input, &mut self.offset);

            if !(can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b',')
            {
                break;
            }
        }

        if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b']' {
            self.fail_child_chain(head);
            return false;
        }

        self.depth -= 1;
        self.builder.set_array(item);
        self.offset += 1;
        true
    }

    fn parse_object(&mut self, item: B::Handle) -> bool {
        if self.depth >= CJSON_NESTING_LIMIT {
            return false;
        }
        self.depth += 1;

        if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b'{' {
            return false;
        }

        self.offset += 1;
        skip_whitespace(self.input, &mut self.offset);
        if can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b'}' {
            self.depth -= 1;
            self.builder.set_object(item);
            self.offset = self.offset.wrapping_add(1);
            return true;
        }

        if !can_access_at_index(self.input, self.offset, 0) {
            self.offset = self.offset.wrapping_sub(1);
            return false;
        }

        self.offset = self.offset.wrapping_sub(1);

        let mut head: Option<B::Handle> = None;
        loop {
            let new_item = match self.builder.new_item() {
                Some(h) => h,
                None => {
                    self.fail_child_chain(head);
                    return false;
                }
            };
            if !self.builder.append_child(item, new_item) {
                self.builder.delete(new_item);
                self.fail_child_chain(head);
                return false;
            }
            if head.is_none() {
                head = Some(new_item);
            }

            if !can_access_at_index(self.input, self.offset, 1) {
                self.fail_child_chain(head);
                return false;
            }

            self.offset = self.offset.wrapping_add(1);
            skip_whitespace(self.input, &mut self.offset);
            if !self.parse_string(new_item) {
                self.fail_child_chain(head);
                return false;
            }
            skip_whitespace(self.input, &mut self.offset);
            self.builder.move_valuestring_to_key(new_item);

            if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b':' {
                self.fail_child_chain(head);
                return false;
            }

            self.offset = self.offset.wrapping_add(1);
            skip_whitespace(self.input, &mut self.offset);
            if !self.parse_value(new_item) {
                self.fail_child_chain(head);
                return false;
            }
            skip_whitespace(self.input, &mut self.offset);

            if !(can_access_at_index(self.input, self.offset, 0) && self.input[self.offset] == b',')
            {
                break;
            }
        }

        if !can_access_at_index(self.input, self.offset, 0) || self.input[self.offset] != b'}' {
            self.fail_child_chain(head);
            return false;
        }

        self.depth -= 1;
        self.builder.set_object(item);
        self.offset += 1;
        true
    }

    /// cJSON `cJSON_Delete(head)` on array/object failure.
    fn fail_child_chain(&mut self, head: Option<B::Handle>) {
        if let Some(head) = head {
            self.builder.delete(head);
        }
    }
}

/// Parse a complete JSON value from `input`.
///
/// On success, returns `(root, end_offset)` where `end_offset` is the first unused
/// byte (cJSON `buffer_at_offset` after the value, without requiring NUL).
///
/// Must match cJSON: trailing junk is allowed unless `require_null_terminated`;
/// whitespace is any byte `<= 32`; nesting limit [`CJSON_NESTING_LIMIT`].
pub fn parse<B: TreeBuilder>(
    builder: &mut B,
    input: &[u8],
    opts: ParseOptions,
) -> Result<(B::Handle, usize), ParseError> {
    if input.is_empty() {
        return Err(ParseError { offset: 0 });
    }

    let item = match builder.new_item() {
        Some(h) => h,
        None => return Err(ParseError { offset: 0 }),
    };

    let mut parser = Parser {
        builder,
        input,
        offset: 0,
        depth: 0,
    };
    skip_utf8_bom(parser.input, &mut parser.offset);
    skip_whitespace(parser.input, &mut parser.offset);

    if !parser.parse_value(item) {
        let offset = parser.offset;
        parser.builder.delete(item);
        return Err(ParseError { offset });
    }

    if opts.require_null_terminated {
        skip_whitespace(parser.input, &mut parser.offset);
        if parser.offset >= parser.input.len() || parser.input[parser.offset] != 0 {
            let offset = parser.offset;
            parser.builder.delete(item);
            return Err(ParseError { offset });
        }
    }

    Ok((item, parser.offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{NumberParser, ParsedNumber, TreeBuilder};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Invalid,
        Null,
        Bool,
        Number,
        String,
        Array,
        Object,
    }

    struct Node {
        kind: Kind,
        valuedouble: f64,
        valueint: i32,
        valuestring: Option<Vec<u8>>,
        key: Option<Vec<u8>>,
        child: Option<usize>,
        next: Option<usize>,
        prev: Option<usize>,
        deleted: bool,
    }

    impl Node {
        fn new() -> Self {
            Self {
                kind: Kind::Invalid,
                valuedouble: 0.0,
                valueint: 0,
                valuestring: None,
                key: None,
                child: None,
                next: None,
                prev: None,
                deleted: false,
            }
        }
    }

    struct MockBuilder {
        nodes: Vec<Node>,
    }

    impl MockBuilder {
        fn new() -> Self {
            Self { nodes: Vec::new() }
        }

        fn valuestring(&self, h: usize) -> Option<&[u8]> {
            self.nodes[h].valuestring.as_deref()
        }

        fn key(&self, h: usize) -> Option<&[u8]> {
            self.nodes[h].key.as_deref()
        }

        fn kind(&self, h: usize) -> Kind {
            self.nodes[h].kind
        }

        fn child(&self, h: usize) -> Option<usize> {
            self.nodes[h].child
        }

        fn next(&self, h: usize) -> Option<usize> {
            self.nodes[h].next
        }
    }

    fn saturate_int(number: f64) -> i32 {
        if number >= i32::MAX as f64 {
            i32::MAX
        } else if number <= i32::MIN as f64 {
            i32::MIN
        } else {
            number as i32
        }
    }

    /// Safe strtod-like prefix parse for tests (no `unsafe`).
    fn parse_number_token(input: &[u8]) -> Option<ParsedNumber> {
        if input.is_empty() {
            return None;
        }
        let mut i = 0usize;
        while i < input.len() && input[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < input.len() && (input[i] == b'+' || input[i] == b'-') {
            i += 1;
        }
        let mut has_digit = false;
        while i < input.len() && input[i].is_ascii_digit() {
            has_digit = true;
            i += 1;
        }
        if i < input.len() && input[i] == b'.' {
            i += 1;
            while i < input.len() && input[i].is_ascii_digit() {
                has_digit = true;
                i += 1;
            }
        }
        if !has_digit {
            return None;
        }
        if i < input.len() && (input[i] == b'e' || input[i] == b'E') {
            let e = i;
            i += 1;
            if i < input.len() && (input[i] == b'+' || input[i] == b'-') {
                i += 1;
            }
            let exp_start = i;
            while i < input.len() && input[i].is_ascii_digit() {
                i += 1;
            }
            if i == exp_start {
                i = e;
            }
        }
        let consumed = i;
        if consumed == 0 {
            return None;
        }
        let s = std::str::from_utf8(&input[..consumed]).ok()?;
        // Rust rejects a leading `+`; strip it for the conversion only.
        let to_parse = s.strip_prefix('+').unwrap_or(s);
        let valuedouble: f64 = to_parse.parse().ok()?;
        Some(ParsedNumber {
            valuedouble,
            valueint: saturate_int(valuedouble),
            consumed,
        })
    }

    impl NumberParser for MockBuilder {
        fn parse_number(&self, input: &[u8]) -> Option<ParsedNumber> {
            parse_number_token(input)
        }
    }

    impl TreeBuilder for MockBuilder {
        type Handle = usize;

        fn new_item(&mut self) -> Option<Self::Handle> {
            let h = self.nodes.len();
            self.nodes.push(Node::new());
            Some(h)
        }

        fn set_null(&mut self, item: Self::Handle) {
            self.nodes[item].kind = Kind::Null;
        }

        fn set_bool(&mut self, item: Self::Handle, value: bool) {
            self.nodes[item].kind = Kind::Bool;
            self.nodes[item].valueint = if value { 1 } else { 0 };
        }

        fn set_number(&mut self, item: Self::Handle, valuedouble: f64, valueint: i32) {
            self.nodes[item].kind = Kind::Number;
            self.nodes[item].valuedouble = valuedouble;
            self.nodes[item].valueint = valueint;
        }

        fn set_array(&mut self, item: Self::Handle) {
            self.nodes[item].kind = Kind::Array;
        }

        fn set_object(&mut self, item: Self::Handle) {
            self.nodes[item].kind = Kind::Object;
        }

        fn set_string<F>(&mut self, item: Self::Handle, cap: usize, fill: F) -> bool
        where
            F: FnOnce(&mut [u8]) -> Option<usize>,
        {
            let mut buf = vec![0u8; cap];
            match fill(&mut buf) {
                Some(used) if used <= cap => {
                    buf.truncate(used);
                    self.nodes[item].valuestring = Some(buf);
                    self.nodes[item].kind = Kind::String;
                    true
                }
                _ => false,
            }
        }

        fn move_valuestring_to_key(&mut self, item: Self::Handle) {
            self.nodes[item].key = self.nodes[item].valuestring.take();
        }

        fn append_child(&mut self, parent: Self::Handle, child: Self::Handle) -> bool {
            if parent == child {
                return false;
            }
            match self.nodes[parent].child {
                None => {
                    self.nodes[parent].child = Some(child);
                    self.nodes[child].prev = Some(child);
                    self.nodes[child].next = None;
                }
                Some(head) => {
                    let last = self.nodes[head].prev.unwrap_or(head);
                    self.nodes[last].next = Some(child);
                    self.nodes[child].prev = Some(last);
                    self.nodes[child].next = None;
                    self.nodes[head].prev = Some(child);
                }
            }
            true
        }

        fn delete(&mut self, item: Self::Handle) {
            if item >= self.nodes.len() || self.nodes[item].deleted {
                return;
            }
            // Detach from any parent so a later delete of the parent is safe.
            for n in &mut self.nodes {
                if n.child == Some(item) {
                    n.child = None;
                }
            }
            self.nodes[item].deleted = true;
            let child = self.nodes[item].child;
            let next = self.nodes[item].next;
            self.nodes[item].child = None;
            self.nodes[item].next = None;
            if let Some(c) = child {
                self.delete(c);
            }
            if let Some(n) = next {
                self.delete(n);
            }
        }

        fn number_parser(&self) -> &dyn NumberParser {
            self
        }
    }

    fn parse_ok(input: &[u8]) -> (MockBuilder, usize, usize) {
        parse_ok_opts(input, false)
    }

    fn parse_ok_opts(input: &[u8], require_null_terminated: bool) -> (MockBuilder, usize, usize) {
        let mut builder = MockBuilder::new();
        let (root, end) = parse(
            &mut builder,
            input,
            ParseOptions {
                require_null_terminated,
            },
        )
        .unwrap_or_else(|e| panic!("parse failed at {} for {:?}", e.offset, input));
        (builder, root, end)
    }

    fn parse_err(input: &[u8], require_null_terminated: bool) -> usize {
        let mut builder = MockBuilder::new();
        match parse(
            &mut builder,
            input,
            ParseOptions {
                require_null_terminated,
            },
        ) {
            Err(e) => e.offset,
            Ok(_) => panic!("expected parse to fail for {:?}", input),
        }
    }

    #[test]
    fn hex4_lower_and_upper() {
        assert_eq!(parse_hex4(b"beef"), 0xBEEF);
        assert_eq!(parse_hex4(b"BEEF"), 0xBEEF);
        assert_eq!(parse_hex4(b"beEF"), 0xBEEF);
        assert_eq!(parse_hex4(b"0000"), 0);
        assert_eq!(parse_hex4(b"ffff"), 0xFFFF);
    }

    #[test]
    fn hex4_invalid_and_short() {
        assert_eq!(parse_hex4(b"xxxx"), 0);
        assert_eq!(parse_hex4(b"ab"), 0);
        assert_eq!(parse_hex4(b""), 0);
        assert_eq!(parse_hex4(b"12GG"), 0);
    }

    #[test]
    fn unescape_utf16_surrogate_pair_cat() {
        let (b, root, end) = parse_ok(br#""\uD83D\udc31""#);
        assert_eq!(b.kind(root), Kind::String);
        assert_eq!(b.valuestring(root), Some("🐱".as_bytes()));
        assert_eq!(end, br#""\uD83D\udc31""#.len());
    }

    #[test]
    fn empty_string() {
        let (b, root, end) = parse_ok(br#""""#);
        assert_eq!(b.kind(root), Kind::String);
        assert_eq!(b.valuestring(root), Some(&b""[..]));
        assert_eq!(end, 2);
    }

    #[test]
    fn invalid_backslash_fails() {
        let _ = parse_err(br#""abc\x""#, false);
        let _ = parse_err(br#""abcdef\123""#, false);
        let _ = parse_err(br#""000000000000000000\"#, false);
    }

    #[test]
    fn nested_arrays_and_objects() {
        let (b, root, _) = parse_ok(br#"{"arr":[1,{"k":true},[]],"n":null}"#);
        assert_eq!(b.kind(root), Kind::Object);

        let arr_item = b.child(root).expect("object child");
        assert_eq!(b.key(arr_item), Some(&b"arr"[..]));
        assert_eq!(b.kind(arr_item), Kind::Array);

        let one = b.child(arr_item).expect("array child");
        assert_eq!(b.kind(one), Kind::Number);
        assert_eq!(b.nodes[one].valueint, 1);

        let obj = b.next(one).expect("second array elem");
        assert_eq!(b.kind(obj), Kind::Object);
        let k = b.child(obj).expect("nested object child");
        assert_eq!(b.key(k), Some(&b"k"[..]));
        assert_eq!(b.kind(k), Kind::Bool);
        assert_eq!(b.nodes[k].valueint, 1);

        let empty_arr = b.next(obj).expect("third array elem");
        assert_eq!(b.kind(empty_arr), Kind::Array);
        assert!(b.child(empty_arr).is_none());

        let n = b.next(arr_item).expect("second object member");
        assert_eq!(b.key(n), Some(&b"n"[..]));
        assert_eq!(b.kind(n), Kind::Null);
    }

    #[test]
    fn trailing_junk_allowed_without_null_terminated() {
        let json = b"[] empty array XD";
        let (b, root, end) = parse_ok(json);
        assert_eq!(b.kind(root), Kind::Array);
        assert_eq!(end, 2);

        let _ = parse_err(b"[]x", true);
        let (b2, root2, end2) = parse_ok_opts(b"[]\0", true);
        assert_eq!(b2.kind(root2), Kind::Array);
        assert_eq!(end2, 2);
    }

    #[test]
    fn literals_numbers_and_empty_containers() {
        let (b, root, _) = parse_ok(b"null");
        assert_eq!(b.kind(root), Kind::Null);

        let (b, root, _) = parse_ok(b"true");
        assert_eq!(b.kind(root), Kind::Bool);
        assert_eq!(b.nodes[root].valueint, 1);

        let (b, root, _) = parse_ok(b"false");
        assert_eq!(b.kind(root), Kind::Bool);
        assert_eq!(b.nodes[root].valueint, 0);

        let (b, root, _) = parse_ok(b"1.5");
        assert_eq!(b.kind(root), Kind::Number);
        assert_eq!(b.nodes[root].valuedouble, 1.5);

        let (b, root, end) = parse_ok(b"[]");
        assert_eq!(b.kind(root), Kind::Array);
        assert!(b.child(root).is_none());
        assert_eq!(end, 2);

        let (b, root, end) = parse_ok(b"{}");
        assert_eq!(b.kind(root), Kind::Object);
        assert!(b.child(root).is_none());
        assert_eq!(end, 2);
    }

    #[test]
    fn empty_input_fails_at_zero() {
        assert_eq!(parse_err(b"", false), 0);
    }

    #[test]
    fn string_escapes_and_slash() {
        let (b, root, _) = parse_ok(br#""\"\\\/\b\f\n\r\t""#);
        assert_eq!(
            b.valuestring(root),
            Some(&b"\"\\/\x08\x0c\n\r\t"[..])
        );
    }

    #[test]
    fn lone_low_surrogate_fails() {
        let _ = parse_err(br#""\uDC00""#, false);
        let _ = parse_err(br#""\uD83D""#, false);
        let _ = parse_err(br#""\uD83D\u0020""#, false);
    }

    #[test]
    fn utf8_bom_and_whitespace() {
        let (b, root, _) = parse_ok(b"\xEF\xBB\xBF{}x");
        assert_eq!(b.kind(root), Kind::Object);
        let (b, root, _) = parse_ok(b"\t\n null");
        assert_eq!(b.kind(root), Kind::Null);
    }

    #[test]
    fn incomplete_containers_fail() {
        let _ = parse_err(b"[1,", false);
        let _ = parse_err(b"{\"a\":", false);
        let _ = parse_err(b"{\0", false);
        let _ = parse_err(b"[\0", false);
        let _ = parse_err(b"{\"a\" 1}", false);
    }

    #[test]
    fn unicode_bmp_escapes() {
        let (b, root, _) = parse_ok("\"\\u20AC\\u732b\"".as_bytes());
        assert_eq!(b.valuestring(root), Some("€猫".as_bytes()));
    }

    #[test]
    fn require_null_terminated_accepts_trailing_whitespace() {
        let (b, root, end) = parse_ok_opts(b"{}\n\0", true);
        assert_eq!(b.kind(root), Kind::Object);
        assert_eq!(end, 3);
    }
}
