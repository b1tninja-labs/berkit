//! Differential test: the iterative reader (`tlv.rs`) against the recursive one it replaced (`oracle.rs`).
//!
//! The claim under test: for every input, in both modes and under every [`Limits`], the two readers give the
//! same RESULT, not just the same verdict. That is compared as follows, for `parse`, `parse_exact`, and then for
//! every node of the tree that `parse` returned (the nodes of the new reader's tree, which must be the old
//! reader's tree):
//!
//! - the `Result`: the same `Ok` or the same [`Error`], class AND offset;
//! - an `Ok` element: the same tag, length form, header, contents, encoding (the same octets at the same
//!   addresses), offset, level, mode and limits, and the same rest after it;
//! - `validate` on the node, and the whole sequence of `children()` (every item, up to and including the first
//!   error), which reads each child with the `bounded` flag of a definite parent.
//!
//! The inputs are the ones of the robustness sweeps (`tests/common/inputs.rs`, shared with `tests/robustness.rs`:
//! 40 real certificates and every hex input of the corpus, mutated; crafted length fields; crafted nesting
//! shapes at and beyond `HARD_MAX_DEPTH`; random bytes) plus random trees of mixed definite and indefinite
//! elements with faults put in on purpose, because the order of faults inside nested indefinite lengths is where
//! an iterative rewrite would differ.
//!
//! The oracle recurses once per level (about 2.7 KiB in a debug build), so the checks run on a thread with a big
//! stack: the iterative reader is under test, not the oracle's use of the stack.

// The library is `no_std`; the tests have `std`.
use std::prelude::v1::*;

use super::{Limits, Tlv, oracle};
use crate::{Error, Mode};
use std::collections::BTreeMap;
use std::thread;

#[allow(dead_code)]
#[path = "../../tests/common/inputs.rs"]
mod inputs;
#[allow(dead_code)]
#[path = "../../tests/common/json.rs"]
mod json;
#[allow(dead_code)]
#[path = "../../tests/common/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../tests/common/sha256.rs"]
mod sha256;

use inputs::{
    LIMITS, Rng, corpus_inputs, crafted_length_inputs, mutate, nest_definite, nest_indefinite,
    nesting_shapes, push_len, random_inputs, random_trees, real_certificates,
};

/// The stack of the thread that runs both readers: the oracle needs about 1.4 MiB for 512 levels in a debug build.
const STACK: usize = 64 * 1024 * 1024;

/// The most nodes of one tree whose `validate` and `children()` are compared (a deep chain has one per level).
const MAX_NODES: usize = 4000;

fn same_octets(a: &[u8], b: &[u8]) -> bool {
    a.as_ptr() == b.as_ptr() && a.len() == b.len()
}

fn same_tlv(a: &Tlv<'_>, b: &Tlv<'_>) -> bool {
    a.tag == b.tag
        && a.length == b.length
        && same_octets(a.header, b.header)
        && same_octets(a.content, b.content)
        && same_octets(a.encoding, b.encoding)
        && a.offset == b.offset
        && a.level == b.level
        && a.mode == b.mode
        && a.limits == b.limits
}

fn same_item(a: &Result<Tlv<'_>, Error>, b: &Result<Tlv<'_>, Error>) -> bool {
    match (a, b) {
        (Ok(x), Ok(y)) => same_tlv(x, y),
        (Err(x), Err(y)) => x == y,
        _ => false,
    }
}

type Parsed<'a> = Result<(Tlv<'a>, &'a [u8]), Error>;

fn same_parse(a: &Parsed<'_>, b: &Parsed<'_>) -> bool {
    match (a, b) {
        (Ok((x, xr)), Ok((y, yr))) => same_tlv(x, y) && same_octets(xr, yr),
        (Err(x), Err(y)) => x == y,
        _ => false,
    }
}

/// What the comparisons saw, so that the test can say it exercised the branches it is about.
#[derive(Default)]
struct Stats {
    /// Input, mode and limits triples compared.
    runs: usize,
    /// Nodes whose `validate` and `children()` were compared.
    nodes: usize,
    /// How `parse` ended, by `Ok` or error class.
    parse: BTreeMap<String, usize>,
    /// How `parse_exact` ended.
    exact: BTreeMap<String, usize>,
    /// How `validate` ended, over all nodes.
    validate: BTreeMap<String, usize>,
    /// Errors met by a `children()` iterator, over all nodes.
    children: BTreeMap<String, usize>,
}

fn label<T>(r: &Result<T, Error>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => format!("{:?}", e.class),
    }
}

fn bump(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_insert(0) += 1;
}

fn hex(b: &[u8]) -> String {
    b.iter().take(200).fold(String::new(), |mut acc, x| {
        use std::fmt::Write;
        let _ = write!(acc, "{x:02x}");
        acc
    })
}

fn check(input: &[u8], mode: Mode, limits: &Limits, stats: &mut Stats) {
    let what = || {
        format!(
            "input {} ({} octets), {mode:?}, {limits:?}",
            hex(input),
            input.len()
        )
    };
    stats.runs += 1;

    let new = Tlv::parse(input, mode, limits);
    let old = oracle::parse(input, mode, limits);
    assert!(
        same_parse(&new, &old),
        "parse differs: new {new:?}, old {old:?}; {}",
        what()
    );
    bump(&mut stats.parse, label(&new));

    let new_exact = Tlv::parse_exact(input, mode, limits);
    let old_exact = oracle::parse_exact(input, mode, limits);
    assert!(
        same_item(&new_exact, &old_exact),
        "parse_exact differs: new {new_exact:?}, old {old_exact:?}; {}",
        what()
    );
    bump(&mut stats.exact, label(&new_exact));

    let Ok((root, _)) = new else { return };
    let mut nodes = vec![root];
    let mut seen = 0;
    while let Some(t) = nodes.pop() {
        seen += 1;
        stats.nodes += 1;
        let new_v = t.validate();
        let old_v = oracle::validate(&t);
        assert_eq!(
            new_v,
            old_v,
            "validate differs at offset {}; {}",
            t.offset(),
            what()
        );
        bump(&mut stats.validate, label(&new_v));

        let new_kids: Vec<_> = t.children().collect();
        let old_kids: Vec<_> = oracle::children(&t).collect();
        assert_eq!(
            new_kids.len(),
            old_kids.len(),
            "children() of the node at {} differ in count: new {new_kids:?}, old {old_kids:?}; {}",
            t.offset(),
            what()
        );
        for (n, o) in new_kids.iter().zip(&old_kids) {
            assert!(
                same_item(n, o),
                "children() of the node at {} differ: new {n:?}, old {o:?}; {}",
                t.offset(),
                what()
            );
            match n {
                Ok(c) if c.is_constructed() && seen + nodes.len() < MAX_NODES => nodes.push(*c),
                Ok(_) => {}
                Err(_) => bump(&mut stats.children, label(n)),
            }
        }
        if seen >= MAX_NODES {
            break;
        }
    }
}

fn check_all(inputs: &[Vec<u8>], stats: &mut Stats) {
    check_all_under(inputs, &LIMITS, stats);
}

fn check_all_under(inputs: &[Vec<u8>], limits: &[Limits], stats: &mut Stats) {
    for input in inputs {
        for mode in [Mode::Ber, Mode::Der] {
            for l in limits {
                check(input, mode, l, stats);
            }
        }
    }
}

/// Mutations of each of `bases`, `per_base` of them, from a fixed seed; the bases come first.
fn with_mutations(bases: &[Vec<u8>], seed: u64, per_base: usize) -> Vec<Vec<u8>> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for b in bases {
        out.push(b.clone());
        for _ in 0..per_base {
            out.push(mutate(b, &mut rng));
        }
    }
    out
}

/// The nesting of `tests/robustness.rs::check_depths`: definite, indefinite and indefinite without EOC, at the
/// depths of its stack child test and around the hard bound, up to 100,000 levels (the definite shape is cut at
/// 20,000: building it copies the whole input once per level).
fn depth_shapes() -> Vec<Vec<u8>> {
    let int = [0x02, 0x01, 0x05];
    let h = Limits::HARD_MAX_DEPTH;
    let mut out = Vec::new();
    for d in [
        1,
        2,
        3,
        4,
        5,
        6,
        62,
        63,
        64,
        65,
        100,
        h - 2,
        h - 1,
        h,
        h + 1,
        2 * h,
        1000,
        4000,
        20_000,
        100_000,
    ] {
        if d <= 20_000 {
            out.push(nest_definite(0x30, d, &int));
        }
        out.push(nest_indefinite(0x30, d, &int, d));
        out.push(nest_indefinite(0x30, d, &int, 0));
        // The input the old stack test failed on: `a0 80` x d, `03 02 00 41`, `00 00` x d.
        out.push(nest_indefinite(0xA0, d, &[0x03, 0x02, 0x00, 0x41], d));
    }
    out
}

fn report(name: &str, inputs: &[Vec<u8>], stats: &Stats) {
    std::eprintln!(
        "differential {name}: {} inputs, {} comparisons of parse and parse_exact, {} nodes compared",
        inputs.len(),
        stats.runs,
        stats.nodes
    );
}

/// Run `f` on a thread with a big stack: the oracle recurses.
fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the iterative and the recursive reader agree");
}

#[test]
fn real_certificates_and_their_mutations() {
    on_big_stack(|| {
        let certs = real_certificates();
        assert_eq!(certs.len(), 40);
        let inputs = with_mutations(&certs, 0x0123_4567_89AB_CDEF, 20);
        assert_eq!(inputs.len(), 40 * 21);
        let mut stats = Stats::default();
        check_all(&inputs, &mut stats);
        report("certificates", &inputs, &stats);
        // The 40 certificates themselves are accepted in BER and DER (three limit sets each).
        assert!(stats.parse["ok"] >= 40 * 2 * 3);
        assert!(stats.exact["ok"] >= 40 * 2);
    });
}

#[test]
fn corpus_inputs_and_their_mutations() {
    on_big_stack(|| {
        let bases = corpus_inputs();
        assert!(
            bases.len() > 800,
            "the corpus has {} hex inputs",
            bases.len()
        );
        let inputs = with_mutations(&bases, 0xFEED_FACE_CAFE_F00D, 3);
        let mut stats = Stats::default();
        check_all(&inputs, &mut stats);
        report("corpus", &inputs, &stats);
        assert!(stats.parse.len() >= 8, "{:?}", stats.parse);
    });
}

#[test]
fn crafted_length_fields_and_random_bytes() {
    on_big_stack(|| {
        let mut inputs = crafted_length_inputs();
        inputs.extend(random_inputs(4000, 0xDEAD_BEEF_0BAD_F00D));
        let mut stats = Stats::default();
        check_all(&inputs, &mut stats);
        report("lengths and random bytes", &inputs, &stats);
        assert!(stats.parse.len() >= 6, "{:?}", stats.parse);
    });
}

#[test]
fn nesting_shapes_at_and_beyond_the_hard_bound() {
    on_big_stack(|| {
        let mut shapes = nesting_shapes();
        shapes.extend(depth_shapes());
        let inputs = with_mutations(&shapes, 0x5EED_0000_0000_0001, 1);
        let mut stats = Stats::default();
        check_all(&inputs, &mut stats);
        report("nesting shapes", &inputs, &stats);
        for class in ["ok", "LimitExceeded", "MissingEoc"] {
            assert!(
                stats.exact.contains_key(class) || stats.parse.contains_key(class),
                "no {class} seen: {:?}",
                stats.parse
            );
        }
    });
}

#[test]
fn random_trees_with_faults_in_nested_definite_and_indefinite_lengths() {
    on_big_stack(|| {
        let trees = random_trees(6000, 0x7EE5_0000_C0DE_0001);
        let inputs = with_mutations(&trees, 0x7EE5_0000_C0DE_0002, 2);
        let mut stats = Stats::default();
        check_all(&inputs, &mut stats);
        report("random trees", &inputs, &stats);
        // The trees must reach the classes where the order of faults matters, inside the nesting.
        for class in [
            "ok",
            "MissingEoc",
            "LengthOverrun",
            "StrayEoc",
            "IndefiniteOnPrimitive",
        ] {
            assert!(
                stats.validate.contains_key(class)
                    || stats.exact.contains_key(class)
                    || stats.children.contains_key(class),
                "no {class} seen: {:?} {:?}",
                stats.exact,
                stats.children
            );
        }
        std::eprintln!("random trees: parse_exact outcomes {:?}", stats.exact);
        std::eprintln!("random trees: children() errors {:?}", stats.children);
    });
}

/// `max_depth` on both sides of each size of the explicit stack (8, 64 and 512 frames, `walk` in `tlv.rs`) and of
/// the hard bound: the deepest input a stack must hold is the one that is refused one level further down.
#[test]
fn max_depth_at_the_sizes_of_the_explicit_stack() {
    on_big_stack(|| {
        let limits: Vec<Limits> = [
            0,
            1,
            2,
            7,
            8,
            9,
            10,
            63,
            64,
            65,
            66,
            511,
            512,
            513,
            usize::MAX,
        ]
        .into_iter()
        .map(|max_depth| Limits {
            max_depth,
            max_content_length: u64::MAX,
        })
        .collect();
        // Nesting of every depth from 1 to 70 and around the hard bound, in the four shapes, plus random trees.
        let mut inputs = Vec::new();
        let int = [0x02, 0x01, 0x05];
        let h = Limits::HARD_MAX_DEPTH;
        for d in (1..=70).chain([h - 1, h, h + 1]) {
            inputs.push(nest_definite(0x30, d, &int));
            inputs.push(nest_indefinite(0x30, d, &int, d));
            inputs.push(nest_indefinite(0x30, d, &int, d - 1));
            // Definite levels inside indefinite ones, and the reverse.
            let mut v = int.to_vec();
            for i in 0..d {
                let mut w = vec![0x30];
                if i % 2 == 0 {
                    push_len(&mut w, v.len());
                } else {
                    w.push(0x80);
                }
                w.extend_from_slice(&v);
                if i % 2 == 1 {
                    w.extend_from_slice(&[0, 0]);
                }
                v = w;
            }
            inputs.push(v);
        }
        inputs.extend(random_trees(2000, 0x7EE5_0000_C0DE_0003));
        let mut stats = Stats::default();
        check_all_under(&inputs, &limits, &mut stats);
        report("max_depth at the stack sizes", &inputs, &stats);
        assert!(stats.parse["ok"] > 1000 && stats.parse["LimitExceeded"] > 1000);
    });
}
