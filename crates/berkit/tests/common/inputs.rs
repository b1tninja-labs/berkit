//! Input generators shared by the robustness sweeps (`robustness.rs`) and the differential test of the iterative
//! reader against the recursive oracle (`src/tlv.rs`, module `oracle`): a deterministic xorshift generator, the
//! mutators, the real certificates and corpus inputs, the crafted nesting shapes and length fields, and the three
//! limit sets. Test-only, std only.

// A generator of hostile bytes truncates integers on purpose.
#![allow(clippy::cast_possible_truncation)]

// The library is `no_std`; this file is also compiled inside it for the differential test of `src/tlv.rs`.
use berkit::Mode;
use berkit::tag::Tag;
use berkit::tlv::{Limits, Tlv};
use std::fs;
use std::path::PathBuf;
#[allow(unused_imports)]
use std::prelude::v1::*;

// ------------------------------------------------------------------------------------------ generator

/// xorshift64*: deterministic, tiny, not for anything but test inputs.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// A value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
}

// ------------------------------------------------------------------------------------------- mutators

pub fn push_len(w: &mut Vec<u8>, n: usize) {
    if n < 128 {
        w.push(n as u8);
    } else {
        let b = n.to_be_bytes();
        let skip = b.iter().take_while(|&&x| x == 0).count();
        w.push(0x80 | (8 - skip) as u8);
        w.extend_from_slice(&b[skip..]);
    }
}

/// The header positions of a tree read in BER: (offset, tag octets, length octets).
pub fn headers(input: &[u8]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let Ok((root, _)) = Tlv::parse(input, Mode::Ber, &Limits::DEFAULT) else {
        return out;
    };
    let mut stack = vec![root.children()];
    let mut seen = 0;
    while let Some(it) = stack.last_mut() {
        match it.next() {
            Some(Ok(c)) => {
                let h = c.header();
                let tl = Tag::decode(h).map_or(1, |(_, n)| n);
                out.push((c.offset(), tl, h.len() - tl));
                seen += 1;
                if c.is_constructed() && seen < 2000 {
                    stack.push(c.children());
                }
            }
            _ => {
                stack.pop();
            }
        }
    }
    out
}

/// Replacement length octets: non-minimal, indefinite, reserved, huge.
pub fn evil_length(rng: &mut Rng, true_len: usize) -> Vec<u8> {
    match rng.below(14) {
        0 => vec![0x81, true_len as u8],
        1 => vec![0x82, (true_len >> 8) as u8, true_len as u8],
        2 => vec![0x80],
        3 => vec![0xFF],
        4 => vec![0x84, 0xFF, 0xFF, 0xFF, 0xFF],
        5 => vec![0x88, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        6 => vec![0x88, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        7 => vec![0x89, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0],
        8 => vec![0x81, 0x00],
        9 => vec![0x7F],
        10 => vec![0x83, 0x00, 0x00, (true_len as u8).wrapping_add(1)],
        11 => vec![(true_len as u8).wrapping_sub(1) & 0x7F],
        12 => {
            let mut v = vec![0xFE];
            v.extend((0..126).map(|_| rng.byte()));
            v
        }
        _ => vec![0x80 | (1 + rng.below(9)) as u8],
    }
}

/// One mutation of `base`.
pub fn mutate(base: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut v = base.to_vec();
    let n = v.len();
    match rng.below(8) {
        0 if n > 0 => {
            for _ in 0..=rng.below(4) {
                let i = rng.below(n);
                if rng.below(2) == 0 {
                    v[i] ^= 1 << rng.below(8);
                } else {
                    v[i] = rng.byte();
                }
            }
        }
        1 => v.truncate(rng.below(n + 1)),
        2 => {
            let at = rng.below(n + 1);
            let extra: Vec<u8> = (0..=rng.below(6)).map(|_| rng.byte()).collect();
            v.splice(at..at, extra);
        }
        3 if n >= 2 => {
            // Duplicate a header (identifier and length octets) at a random place.
            let hdr = &base[..(2 + rng.below(4)).min(n)];
            let at = rng.below(n + 1);
            v.splice(at..at, hdr.iter().copied());
        }
        4 if n > 0 => {
            // Duplicate a random slice.
            let a = rng.below(n);
            let b = (a + 1 + rng.below(n - a)).min(n);
            let part = base[a..b].to_vec();
            let at = rng.below(n + 1);
            v.splice(at..at, part);
        }
        5..=7 => {
            let hs = headers(base);
            if hs.is_empty() {
                if n >= 2 {
                    let ev = evil_length(rng, n);
                    v.splice(1..2, ev);
                }
            } else {
                let (off, tl, ll) = hs[rng.below(hs.len())];
                let ev = evil_length(rng, n);
                v.splice(off + tl..off + tl + ll, ev);
            }
        }
        _ => v.push(rng.byte()),
    }
    v
}

// ------------------------------------------------------------------------------------------- the data

pub fn real_certificates() -> Vec<Vec<u8>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/real");
    let mut names: Vec<_> = fs::read_dir(dir.clone())
        .expect("data/real")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            PathBuf::from(n)
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("der"))
        })
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| fs::read(dir.join(n)).expect("read"))
        .collect()
}

pub fn corpus_inputs() -> Vec<Vec<u8>> {
    let corpus = model::load().expect("corpus loads");
    corpus
        .cases
        .into_iter()
        .filter_map(|c| match c.input {
            Input::Hex(h) => Some(h),
            _ => None,
        })
        .collect()
}

use super::model::{self, Input};

/// `depth` levels of `tag` around `core`, definite lengths.
pub fn nest_definite(tag: u8, depth: usize, core: &[u8]) -> Vec<u8> {
    let mut v = core.to_vec();
    for _ in 0..depth {
        let mut w = vec![tag];
        push_len(&mut w, v.len());
        w.extend_from_slice(&v);
        v = w;
    }
    v
}

/// `depth` levels of `tag` with the indefinite length around `core`, closed by `eocs` end-of-contents pairs.
pub fn nest_indefinite(tag: u8, depth: usize, core: &[u8], eocs: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..depth {
        v.extend_from_slice(&[tag, 0x80]);
    }
    v.extend_from_slice(core);
    for _ in 0..eocs.min(depth) {
        v.extend_from_slice(&[0, 0]);
    }
    v
}

pub const DEPTHS: &[usize] = &[1, 2, 3, 15, 16, 17, 63, 64, 65, 66, 256, 511, 512, 513];

pub fn nesting_shapes() -> Vec<Vec<u8>> {
    let int = [0x02, 0x01, 0x05];
    let mut out = Vec::new();
    for &d in DEPTHS {
        for tag in [0x30u8, 0xA0, 0x24] {
            out.push(nest_definite(tag, d, &int));
            out.push(nest_definite(tag, d, &[]));
            out.push(nest_indefinite(tag, d, &int, d)); // well formed
            out.push(nest_indefinite(tag, d, &int, 0)); // no EOC at all
            out.push(nest_indefinite(tag, d, &int, d / 2)); // half of them
            out.push(nest_indefinite(tag, d, &[], 0));
            out.push(nest_indefinite(tag, d, &[0x04, 0x01, 0x41], d)); // constructed strings of one segment
            out.push(nest_indefinite(tag, d, &[0x03, 0x02, 0x00, 0x41], d));
        }
        // Alternating definite and indefinite levels: a definite SEQUENCE inside an indefinite one, and so on.
        let mut v = int.to_vec();
        for i in 0..d {
            if i % 2 == 0 {
                let mut w = vec![0x30];
                push_len(&mut w, v.len());
                w.extend_from_slice(&v);
                v = w;
            } else {
                let mut w = vec![0x30, 0x80];
                w.extend_from_slice(&v);
                w.extend_from_slice(&[0, 0]);
                v = w;
            }
        }
        out.push(v);
    }
    out
}

/// The three limit sets every sweep runs under: the defaults, a tight set, and none (the hard bound only).
pub const LIMITS: [Limits; 3] = [
    Limits::DEFAULT,
    Limits {
        max_depth: 4,
        max_content_length: 16,
    },
    Limits {
        max_depth: usize::MAX,
        max_content_length: u64::MAX,
    },
];

/// Crafted length fields: every first length octet with a tail and a body, and every definite length around
/// the true one.
pub fn crafted_length_inputs() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let tags: [&[u8]; 8] = [
        &[0x30],
        &[0x04],
        &[0x24],
        &[0x02],
        &[0x1F, 0x7F],
        &[0xBF, 0x81, 0x00],
        &[0x31],
        &[0x00],
    ];
    let firsts = [
        0x00, 0x05, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x88, 0x89, 0xFE, 0xFF,
    ];
    let tails: [&[u8]; 12] = [
        &[],
        &[0x00],
        &[0x05],
        &[0x00, 0x05],
        &[0xFF],
        &[0xFF, 0xFF],
        &[0x7F, 0xFF, 0xFF, 0xFF],
        &[0xFF; 8],
        &[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        &[0x01, 0x00, 0x00, 0x00],
        &[0x00; 8],
        &[0x80; 4],
    ];
    let bodies: [&[u8]; 4] = [
        &[],
        &[0x02, 0x01, 0x05],
        &[0x04, 0x00, 0x00, 0x00],
        &[0x30, 0x80],
    ];
    for tag in tags {
        for first in firsts {
            for tail in tails {
                for body in bodies {
                    let mut input = tag.to_vec();
                    input.push(first);
                    input.extend_from_slice(tail);
                    input.extend_from_slice(body);
                    out.push(input);
                }
            }
        }
    }
    // Every definite length around the true one, for a SEQUENCE of three octets.
    for declared in 0..=130u8 {
        let input = [0x30, declared, 0x02, 0x01, 0x05];
        out.push(input.to_vec());
        let input = [0x30, 0x81, declared, 0x02, 0x01, 0x05];
        out.push(input.to_vec());
    }
    out
}

/// Random bytes that are mostly structure: identifiers, lengths, EOC and a few arbitrary octets.
pub fn random_inputs(count: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = Rng::new(seed);
    let pool = [
        0x30u8, 0x31, 0x02, 0x04, 0x24, 0x03, 0x23, 0x06, 0x0C, 0x17, 0x18, 0x80, 0x00, 0x00, 0x01,
        0x05, 0x81, 0x82, 0xFF, 0x1F, 0x7F, 0xA0, 0xBF, 0x09, 0x0A,
    ];
    (0..count)
        .map(|_| {
            let len = rng.below(48);
            (0..len)
                .map(|_| {
                    if rng.below(4) == 0 {
                        rng.byte()
                    } else {
                        pool[rng.below(pool.len())]
                    }
                })
                .collect()
        })
        .collect()
}

/// Random trees of mixed definite and indefinite elements, with faults put in on purpose (a missing or an extra
/// end-of-contents, a definite length one too long or too short, a primitive element with the indefinite form,
/// stray octets). They are the inputs where the ORDER of faults inside nested indefinite lengths shows.
pub fn random_trees(count: usize, seed: u64) -> Vec<Vec<u8>> {
    const TAGS: [&[u8]; 8] = [
        &[0x30],
        &[0x31],
        &[0xA0],
        &[0xA1],
        &[0x24],
        &[0x23],
        &[0xBF, 0x81, 0x00],
        &[0x1F, 0x20],
    ];
    const LEAVES: [&[u8]; 8] = [
        &[0x02],
        &[0x04],
        &[0x05],
        &[0x0C],
        &[0x80],
        &[0x03],
        &[0x06],
        &[0x1F, 0x21],
    ];

    fn tree(rng: &mut Rng, depth: usize, out: &mut Vec<u8>) {
        let pick = if depth == 0 {
            rng.below(3)
        } else {
            rng.below(12)
        };
        match pick {
            0..=2 => {
                // A primitive element, now and then with the indefinite form (a fault).
                out.extend_from_slice(LEAVES[rng.below(LEAVES.len())]);
                if rng.below(24) == 0 {
                    out.push(0x80);
                    return;
                }
                let n = rng.below(5);
                out.push(n as u8);
                out.extend((0..n).map(|_| rng.byte()));
            }
            3..=6 => {
                // Constructed, definite length (sometimes one octet wrong).
                let mut body = Vec::new();
                for _ in 0..rng.below(4) {
                    tree(rng, depth - 1, &mut body);
                }
                out.extend_from_slice(TAGS[rng.below(TAGS.len())]);
                let mut n = body.len();
                match rng.below(14) {
                    0 => n += 1,
                    1 => n = n.saturating_sub(1),
                    _ => {}
                }
                push_len(out, n);
                out.extend_from_slice(&body);
            }
            7..=10 => {
                // Constructed, indefinite length, usually closed.
                out.extend_from_slice(TAGS[rng.below(TAGS.len())]);
                out.push(0x80);
                for _ in 0..rng.below(4) {
                    tree(rng, depth - 1, out);
                }
                match rng.below(12) {
                    0 => {}
                    1 => out.extend_from_slice(&[0, 0, 0, 0]),
                    2 => out.extend_from_slice(&[0, 1]),
                    3 => out.push(0),
                    _ => out.extend_from_slice(&[0, 0]),
                }
            }
            _ => out.extend((0..rng.below(5)).map(|_| rng.byte())),
        }
    }

    let mut rng = Rng::new(seed);
    (0..count)
        .map(|i| {
            let mut v = Vec::new();
            let depth = if i % 16 == 0 { 40 } else { 1 + rng.below(10) };
            tree(&mut rng, depth, &mut v);
            // Octets after the element, for `trailing_data`, now and then.
            if rng.below(8) == 0 {
                v.extend((0..=rng.below(3)).map(|_| rng.byte()));
            }
            v
        })
        .collect()
}
