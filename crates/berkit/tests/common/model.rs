//! The case model: the corpus manifest, the cases, and the tree form (`corpus/format.md`).
//!
//! Files are read as BYTES and never normalised, so the manifest's sha256 values hold.

use super::json::{self, Json};
use super::sha256::sha256_hex;
use berkit::ErrorClass;
use berkit::length::Length;
use berkit::tag::{Class, Tag};
use berkit::tlv::{self, Tlv};
use berkit::{Error, Mode};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- tree

/// How the length octets are written. `Long(k)` is `k` octets after the initial one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LenForm {
    Short,
    Long(u8),
    Indefinite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Children(Vec<Node>),
    Content(Vec<u8>),
}

/// One node of the corpus tree form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub class: Class,
    pub constructed: bool,
    pub tag: u64,
    pub length: LenForm,
    pub body: Body,
}

fn class_from_name(name: &str) -> Option<Class> {
    Some(match name {
        "universal" => Class::Universal,
        "application" => Class::Application,
        "context" => Class::ContextSpecific,
        "private" => Class::Private,
        _ => return None,
    })
}

impl Node {
    pub fn from_json(j: &Json) -> Result<Node, String> {
        let class = j
            .get("class")
            .and_then(Json::as_str)
            .and_then(class_from_name)
            .ok_or("node: bad class")?;
        let constructed = j
            .get("constructed")
            .and_then(Json::as_bool)
            .ok_or("node: bad constructed")?;
        let tag = j.get("tag").and_then(Json::as_u64).ok_or("node: bad tag")?;
        let len = j.get("length").ok_or("node: no length")?;
        let length = match len.get("form").and_then(Json::as_str) {
            Some("short") => LenForm::Short,
            Some("indefinite") => LenForm::Indefinite,
            Some("long") => {
                let k = len
                    .get("octets")
                    .and_then(Json::as_u64)
                    .and_then(|k| u8::try_from(k).ok())
                    .ok_or("node: long form without octets")?;
                LenForm::Long(k)
            }
            _ => return Err("node: bad length form".into()),
        };
        let body = if constructed {
            let kids = j
                .get("children")
                .and_then(Json::as_array)
                .ok_or("node: constructed without children")?;
            Body::Children(kids.iter().map(Node::from_json).collect::<Result<_, _>>()?)
        } else {
            let hex = j
                .get("content")
                .and_then(Json::as_str)
                .ok_or("node: primitive without content")?;
            Body::Content(unhex(hex)?)
        };
        Ok(Node {
            class,
            constructed,
            tag,
            length,
            body,
        })
    }

    /// The tree the reader found. The element must already have been validated.
    pub fn from_tlv(t: &Tlv<'_>) -> Result<Node, Error> {
        let tag = t.tag();
        let header = t.header();
        let first = header[tag.encoded_len()];
        let length = match first {
            0x80 => LenForm::Indefinite,
            0..=0x7F => LenForm::Short,
            n => LenForm::Long(n & 0x7F),
        };
        let body = if tag.constructed {
            let mut kids = Vec::new();
            for c in t.children() {
                kids.push(Node::from_tlv(&c?)?);
            }
            Body::Children(kids)
        } else {
            Body::Content(t.content().to_vec())
        };
        Ok(Node {
            class: tag.class,
            constructed: tag.constructed,
            tag: tag.number,
            length,
            body,
        })
    }

    /// Equal in every field except the length form (the BER roundtrip comparison, `format.md`).
    pub fn same_ignoring_length(&self, other: &Node) -> bool {
        self.class == other.class
            && self.constructed == other.constructed
            && self.tag == other.tag
            && match (&self.body, &other.body) {
                (Body::Content(a), Body::Content(b)) => a == b,
                (Body::Children(a), Body::Children(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same_ignoring_length(y))
                }
                _ => false,
            }
    }

    /// Serialise with the crate's writer. With `strict` the node's length form is reproduced exactly, including a
    /// long form with surplus length octets (`tlv::write_header_long`, a BER sender's choice); otherwise a long
    /// form with surplus length octets is written minimally (BER roundtrip, where bytes need not match).
    pub fn encode(&self, strict: bool, out: &mut Vec<u8>) -> Result<(), String> {
        let tag = Tag::new(self.class, self.constructed, self.tag);
        let mut inner = Vec::new();
        match &self.body {
            Body::Content(c) => inner.extend_from_slice(c),
            Body::Children(kids) => {
                for k in kids {
                    k.encode(strict, &mut inner)?;
                }
            }
        }
        let n = inner.len() as u64;
        let minimal = Length::Definite(n).encoded_len() - 1; // 0 for the short form
        let mut long_octets = None;
        let length = match self.length {
            LenForm::Indefinite => Length::Indefinite,
            LenForm::Short if n >= 128 => return Err("short form cannot hold the length".into()),
            LenForm::Short => Length::Definite(n),
            LenForm::Long(k) => {
                if strict && usize::from(k) != minimal {
                    long_octets = Some(usize::from(k));
                }
                Length::Definite(n)
            }
        };
        let mut head = [0u8; 160];
        let h = match long_octets {
            Some(k) => tlv::write_header_long(&tag, n, k, &mut head),
            None => tlv::write_header(&tag, length, &mut head),
        }
        .map_err(|e| e.to_string())?;
        out.extend_from_slice(&head[..h]);
        out.extend_from_slice(&inner);
        if length == Length::Indefinite {
            let mut eoc = [0u8; 2];
            tlv::write_eoc(&mut eoc).map_err(|e| e.to_string())?;
            out.extend_from_slice(&eoc);
        }
        Ok(())
    }

    /// A compact one-line rendering for failure messages.
    pub fn describe(&self) -> String {
        let c = match self.class {
            Class::Universal => 'U',
            Class::Application => 'A',
            Class::ContextSpecific => 'C',
            Class::Private => 'P',
        };
        let len = match self.length {
            LenForm::Short => "s".to_string(),
            LenForm::Long(k) => format!("l{k}"),
            LenForm::Indefinite => "i".to_string(),
        };
        let body = match &self.body {
            Body::Content(b) if b.len() > 12 => format!("{}..({})", hex(&b[..12]), b.len()),
            Body::Content(b) => hex(b),
            Body::Children(k) => {
                let parts: Vec<String> = k.iter().map(Node::describe).collect();
                format!("{{{}}}", parts.join(" "))
            }
        };
        format!(
            "{c}{}{}/{len}:{body}",
            self.tag,
            if self.constructed { "c" } else { "" }
        )
    }
}

pub fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(format!("bad hex {s:?}"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| format!("bad hex {s:?}")))
        .collect()
}

// ---------------------------------------------------------------- cases

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Decode,
    Encode,
    Roundtrip,
}

#[derive(Debug)]
pub enum Input {
    Hex(Vec<u8>),
    Tree(Node),
    Typed { ty: String, value: Json },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Tree(Node),
    Hex(Vec<u8>),
    /// The `ErrorClass` NAME.
    Error(String),
    /// A decode that succeeds with this decoded value (`corpus/format.md`, revision 1.2), optionally with the tree
    /// beside it.
    Value {
        tree: Option<Node>,
        value: Json,
    },
}

#[derive(Debug)]
pub struct Case {
    pub id: String,
    pub op: Op,
    pub ty: Option<String>,
    pub input: Input,
    /// One entry per mode the case names.
    pub expect: Vec<(Mode, Outcome)>,
    pub policy: Option<String>,
    /// The century pivot of a `UTCTime` value (revision 1.2); `None` leaves it to the runner.
    pub pivot: Option<u8>,
}

fn parse_outcome(j: &Json) -> Result<Outcome, String> {
    if let Some(v) = j.get("value") {
        let tree = match j.get("tree") {
            Some(t) => Some(Node::from_json(t)?),
            None => None,
        };
        Ok(Outcome::Value {
            tree,
            value: v.clone(),
        })
    } else if let Some(t) = j.get("tree") {
        Ok(Outcome::Tree(Node::from_json(t)?))
    } else if let Some(h) = j.get("hex").and_then(Json::as_str) {
        Ok(Outcome::Hex(unhex(h)?))
    } else if let Some(e) = j.get("error").and_then(Json::as_str) {
        if ErrorClass::from_name(e).is_none() {
            return Err(format!("unknown error class {e}"));
        }
        Ok(Outcome::Error(e.to_string()))
    } else {
        Err("outcome has none of tree, hex, error".into())
    }
}

impl Case {
    pub fn from_json(j: &Json) -> Result<Case, String> {
        let id = j
            .get("id")
            .and_then(Json::as_str)
            .ok_or("case without id")?
            .to_string();
        let ctx = |e: String| format!("{id}: {e}");
        let op = match j.get("op").and_then(Json::as_str) {
            Some("decode") => Op::Decode,
            Some("encode") => Op::Encode,
            Some("roundtrip") => Op::Roundtrip,
            _ => return Err(ctx("bad op".into())),
        };
        let ty = match j.get("type") {
            Some(Json::Str(s)) => Some(s.clone()),
            Some(Json::Null) | None => None,
            _ => return Err(ctx("bad type".into())),
        };
        let inp = j.get("input").ok_or_else(|| ctx("no input".into()))?;
        let input = if let Some(h) = inp.get("hex").and_then(Json::as_str) {
            Input::Hex(unhex(h).map_err(ctx)?)
        } else if let Some(t) = inp.get("tree") {
            Input::Tree(Node::from_json(t).map_err(ctx)?)
        } else if let Some(t) = inp.get("typed") {
            Input::Typed {
                ty: t
                    .get("type")
                    .and_then(Json::as_str)
                    .ok_or_else(|| ctx("typed without type".into()))?
                    .to_string(),
                value: t
                    .get("value")
                    .cloned()
                    .ok_or_else(|| ctx("typed without value".into()))?,
            }
        } else {
            return Err(ctx("input has none of hex, tree, typed".into()));
        };
        let exp = j
            .get("expect")
            .and_then(Json::as_object)
            .ok_or_else(|| ctx("no expect".into()))?;
        let mut expect = Vec::new();
        for (key, val) in exp {
            let out = parse_outcome(val).map_err(ctx)?;
            match key.as_str() {
                "both" => {
                    expect.push((Mode::Ber, parse_outcome(val).map_err(ctx)?));
                    expect.push((Mode::Der, out));
                }
                "ber" => expect.push((Mode::Ber, out)),
                "der" => expect.push((Mode::Der, out)),
                other => return Err(ctx(format!("unknown expect key {other}"))),
            }
        }
        let pivot = match j.get("pivot") {
            Some(p) => Some(
                p.as_u64()
                    .and_then(|p| u8::try_from(p).ok())
                    .ok_or_else(|| ctx("bad pivot".into()))?,
            ),
            None => None,
        };
        Ok(Case {
            id,
            op,
            ty,
            input,
            expect,
            policy: j.get("policy").and_then(Json::as_str).map(str::to_string),
            pivot,
        })
    }
}

// ---------------------------------------------------------------- manifest

#[derive(Debug)]
pub struct Limits {
    pub depth: usize,
    pub length: u64,
    pub tag_number: u64,
}

#[derive(Debug)]
pub struct Corpus {
    pub version: String,
    pub limits: Limits,
    pub cases: Vec<Case>,
    /// (file, expected sha256, actual sha256)
    pub hashes: Vec<(String, String, String)>,
}

pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn read_json(path: &Path) -> Result<(Vec<u8>, Json), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let j = json::parse(text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((bytes, j))
}

fn limit_u64(l: &Json, key: &str) -> Result<u64, String> {
    // The manifest writes the large limits as decimal strings.
    match l.get(key) {
        Some(Json::Str(s)) => s.parse().map_err(|_| format!("limit {key}")),
        Some(n) => n.as_u64().ok_or_else(|| format!("limit {key}")),
        None => Err(format!("limit {key} missing")),
    }
}

pub fn load() -> Result<Corpus, String> {
    let dir = corpus_dir();
    let (_, manifest) = read_json(&dir.join("corpus.json"))?;
    let version = manifest
        .get("corpus_version")
        .and_then(Json::as_str)
        .ok_or("no corpus_version")?
        .to_string();
    if !version.starts_with("0.") {
        return Err(format!("written for corpus 0.x, found {version}"));
    }
    if manifest.get("corpus_format").and_then(Json::as_str) != Some("1") {
        return Err("corpus_format is not 1".into());
    }
    let l = manifest.get("limits").ok_or("no limits")?;
    let limits = Limits {
        depth: usize::try_from(
            l.get("max_depth")
                .and_then(Json::as_u64)
                .ok_or("max_depth")?,
        )
        .map_err(|_| "max_depth")?,
        length: limit_u64(l, "max_length")?,
        tag_number: limit_u64(l, "max_tag_number")?,
    };
    let files = manifest
        .get("files")
        .and_then(Json::as_object)
        .ok_or("no files")?;
    let mut cases = Vec::new();
    let mut hashes = Vec::new();
    for (name, want) in files {
        let (bytes, j) = read_json(&dir.join(name))?;
        hashes.push((
            name.clone(),
            want.as_str().ok_or("hash not a string")?.to_string(),
            sha256_hex(&bytes),
        ));
        for c in j
            .get("cases")
            .and_then(Json::as_array)
            .ok_or_else(|| format!("{name}: no cases"))?
        {
            cases.push(Case::from_json(c)?);
        }
    }
    Ok(Corpus {
        version,
        limits,
        cases,
        hashes,
    })
}
