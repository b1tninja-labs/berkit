//! A small JSON reader for the corpus files: test-only, std only.
//!
//! Objects keep their key order, numbers keep their source text (the corpus writes tag numbers up to 2^64-1
//! as JSON numbers and the manifest writes the large limits as strings), strings take the usual escapes
//! including `\u` surrogate pairs.

use std::fmt;

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number exactly as written.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// A parse failure with the byte offset where it was noticed.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub why: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.offset, self.why)
    }
}

impl Json {
    /// The value of `key` in an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(o) => Some(o),
            _ => None,
        }
    }

    /// The number as an unsigned 64-bit integer, from its text.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(text) => text.parse().ok(),
            _ => None,
        }
    }
}

/// Parse one JSON document; anything but white space after it is an error.
pub fn parse(text: &str) -> Result<Json, ParseError> {
    let mut p = Parser {
        s: text.as_bytes(),
        pos: 0,
        depth: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.pos != p.s.len() {
        return Err(p.err("data after the document"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    depth: usize,
}

/// Nesting deeper than this is refused so that hostile input cannot overflow the stack.
const MAX_DEPTH: usize = 200;

impl Parser<'_> {
    fn err(&self, why: &'static str) -> ParseError {
        ParseError {
            offset: self.pos,
            why,
        }
    }

    fn ws(&mut self) {
        while matches!(self.s.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Json, ParseError> {
        match self.s.get(self.pos) {
            None => Err(self.err("unexpected end")),
            Some(b'{') => self.nested(Self::object),
            Some(b'[') => self.nested(Self::array),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') if self.eat("true") => Ok(Json::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Json::Bool(false)),
            Some(b'n') if self.eat("null") => Ok(Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn nested(&mut self, f: fn(&mut Self) -> Result<Json, ParseError>) -> Result<Json, ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.pos;
        while matches!(
            self.s.get(self.pos),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.pos]).expect("ASCII");
        let digits = text.trim_start_matches('-');
        if digits.is_empty() || !digits.as_bytes()[0].is_ascii_digit() {
            return Err(self.err("malformed number"));
        }
        Ok(Json::Num(text.to_string()))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let digits = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("short \\u escape"))?;
        let mut v = 0;
        for &d in digits {
            let n = char::from(d)
                .to_digit(16)
                .ok_or_else(|| self.err("bad \\u escape"))?;
            v = v * 16 + n;
        }
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // the opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(&b) = self.s.get(self.pos) else {
                return Err(self.err("unterminated string"));
            };
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.pos) else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return Err(self.err("lone high surrogate"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("bad low surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            char::from_u32(cp).ok_or_else(|| self.err("bad code point"))?
                        }
                        _ => return Err(self.err("unknown escape")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1F => return Err(self.err("control character in string")),
                _ => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8"))
    }

    fn array(&mut self) -> Result<Json, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("]") {
                return Ok(Json::Arr(items));
            }
            return Err(self.err("expected , or ]"));
        }
    }

    fn object(&mut self) -> Result<Json, ParseError> {
        self.pos += 1;
        let mut members = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Json::Obj(members));
        }
        loop {
            self.ws();
            if self.s.get(self.pos) != Some(&b'"') {
                return Err(self.err("expected a key"));
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(":") {
                return Err(self.err("expected :"));
            }
            self.ws();
            members.push((key, self.value()?));
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("}") {
                return Ok(Json::Obj(members));
            }
            return Err(self.err("expected , or }"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(parse("null"), Ok(Json::Null));
        assert_eq!(parse(" true "), Ok(Json::Bool(true)));
        assert_eq!(parse("false"), Ok(Json::Bool(false)));
        assert_eq!(parse("-12.5e3"), Ok(Json::Num("-12.5e3".into())));
    }

    #[test]
    fn numbers_keep_their_text_beyond_two_to_the_53() {
        let v = parse("18446744073709551615").unwrap();
        assert_eq!(v, Json::Num("18446744073709551615".into()));
        assert_eq!(v.as_u64(), Some(u64::MAX));
        assert_eq!(parse("18446744073709551616").unwrap().as_u64(), None);
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(parse(r#""a\"b\\c\/d""#), Ok(Json::Str("a\"b\\c/d".into())));
        assert_eq!(
            parse(r#""\n\t\r\b\f""#),
            Ok(Json::Str("\n\t\r\u{8}\u{c}".into()))
        );
        assert_eq!(parse(r#""\u00e9""#), Ok(Json::Str("\u{e9}".into())));
        // A surrogate pair is one code point: U+1F600.
        assert_eq!(
            parse(r#""\ud83d\ude00""#),
            Ok(Json::Str("\u{1F600}".into()))
        );
        // A multi-byte character written raw (the corpus holds the section sign this way).
        assert_eq!(parse("\"\u{a7}8.1\""), Ok(Json::Str("\u{a7}8.1".into())));
    }

    #[test]
    fn arrays_and_objects_keep_order() {
        let v = parse(r#" {"b": [1, "x", null], "a": {}, "c": []} "#).unwrap();
        let keys: Vec<&str> = v
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["b", "a", "c"]);
        assert_eq!(v.get("b").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(v.get("a"), Some(&Json::Obj(vec![])));
        assert_eq!(v.get("missing"), None);
    }

    #[test]
    fn malformed_input_is_refused() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\"}",
            "{\"a\":1,}",
            "tru",
            "\"abc",
            "\"\\x\"",
            "\"\\ud83d\"",
            "1 2",
            "[1 2]",
            "-",
            "\"\u{1}\"",
            "\"\\u12\"",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn nesting_is_bounded() {
        let deep = "[".repeat(1000) + &"]".repeat(1000);
        assert!(parse(&deep).is_err());
        let ok = "[".repeat(100) + &"]".repeat(100);
        assert!(parse(&ok).is_ok());
    }
}
