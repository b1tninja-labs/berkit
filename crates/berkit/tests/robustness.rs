//! Adversarial robustness: the library is `no_std` and allocation-free, and must never panic, overflow, index
//! out of bounds, read past its input or loop forever, whatever the input.
//!
//! Inputs come from a deterministic mutation sweep (fixed seeds, bounded iterations, a small xorshift generator
//! written here, std only): the 40 real certificates and every hex input of the corpus, mutated by bit flips,
//! truncation, insertion, header duplication and length-field corruption, plus crafted deep nesting, indefinite
//! chains without end-of-contents and random bytes.
//!
//! Every input is run on a worker thread with a 4 MiB stack and a watchdog: the caller waits 2 seconds for
//! each input, so a hang FAILS the test (naming the input) instead of hanging the suite, and a panic is
//! reported with the input that caused it. What is checked on each input:
//!
//! - it returns, and does not panic (the worker's panic is turned into a failure that names the input);
//! - every error offset is at most the length of the slice the function was given;
//! - a parsed element is a sub-slice of the input: its encoding lies inside the input at `offset()`, the header
//!   is its prefix, the contents follow the header, and header + contents (+ 2 for the indefinite form) is the
//!   whole encoding;
//! - the children of a tree are contiguous, in order and fill the parent's contents exactly when no fault is
//!   met, `validate` agrees with a walk of the children, and an iterator that returned an error ends.
//!
//! The content functions are run on the raw input, and on the contents of nodes of every parsed tree.

// A generator of hostile bytes truncates integers on purpose, and one battery calls every content function.
#![allow(clippy::cast_possible_truncation, clippy::too_many_lines)]

#[path = "common/cert_schema.rs"]
mod cert_schema;
#[allow(dead_code)]
mod common;

use berkit::length::{self, Length};
use berkit::strings::{self, StringType};
use berkit::tag::Tag;
use berkit::tlv::{Children, Limits, Tlv};
use berkit::typed::{self, Context, Field, Leaves, Presence, Prim, Schema};
use berkit::{
    Error, ErrorClass, Mode, bitstring, boolean, integer, null, octetstring, oid, real, time,
};
use cert_schema::CERTIFICATE;
use common::inputs::{
    LIMITS, Rng, corpus_inputs, crafted_length_inputs, mutate, nest_definite, nest_indefinite,
    nesting_shapes, random_inputs, real_certificates,
};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// How long one input may take before the watchdog calls it a hang.
const WATCHDOG: Duration = Duration::from_secs(2);
/// The stack of the sweep worker. The reader does not recurse (its stack use is the same at every depth, see
/// `nesting_beyond_max_depth_is_refused_and_fits_a_small_stack`), so the sweeps run on the small stack too.
const WORKER_STACK: usize = SMALL_STACK;
/// A small stack: what an embedded target or a 1 MiB Windows main thread leaves, less a good margin.
const SMALL_STACK: usize = 256 * 1024;

// ------------------------------------------------------------------------------------------- watchdog

fn hex(b: &[u8]) -> String {
    let shown: String = b.iter().take(300).fold(String::new(), |mut acc, x| {
        use std::fmt::Write;
        let _ = write!(acc, "{x:02x}");
        acc
    });
    if b.len() > 300 {
        format!("{shown}... ({} octets)", b.len())
    } else {
        format!("{shown} ({} octets)", b.len())
    }
}

/// A worker thread that runs `f` on each input sent to it, with a watchdog on every input.
struct Runner {
    send: Option<mpsc::Sender<Vec<u8>>>,
    done: mpsc::Receiver<()>,
    handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
}

impl Runner {
    fn new(timeout: Duration, f: fn(&[u8])) -> Self {
        let (send, inbox) = mpsc::channel::<Vec<u8>>();
        let (reply, done) = mpsc::channel::<()>();
        let handle = thread::Builder::new()
            .stack_size(WORKER_STACK)
            .spawn(move || {
                for input in inbox {
                    f(&input);
                    if reply.send(()).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn worker");
        Self {
            send: Some(send),
            done,
            handle: Some(handle),
            timeout,
        }
    }

    /// Run the function on `input`; panics (failing the test) on a hang or a panic in the function.
    fn run(&mut self, label: &str, input: &[u8]) {
        self.send
            .as_ref()
            .expect("open")
            .send(input.to_vec())
            .expect("worker alive");
        match self.done.recv_timeout(self.timeout) {
            Ok(()) => {}
            Err(RecvTimeoutError::Timeout) => {
                // The worker is stuck in the library; it is left behind, the process exits at the end.
                panic!(
                    "HANG ({label}): no answer in {:?} for {}",
                    self.timeout,
                    hex(input)
                );
            }
            Err(RecvTimeoutError::Disconnected) => {
                let why = self
                    .handle
                    .take()
                    .and_then(|h| h.join().err())
                    .map(|p| {
                        p.downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                panic!("PANIC ({label}) on {}: {why}", hex(input));
            }
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.send = None;
    }
}

// ------------------------------------------------------------------------------------- the invariants

fn bounded(e: &Error, len: usize, what: &str) {
    assert!(
        e.offset <= len,
        "{what}: error {e} has an offset past the {len} octets given"
    );
}

/// Where `sub` lies inside `input`, if it does.
fn offset_in(input: &[u8], sub: &[u8]) -> Option<usize> {
    let a = input.as_ptr() as usize;
    let b = sub.as_ptr() as usize;
    (b >= a && b + sub.len() <= a + input.len()).then(|| b - a)
}

/// The structural facts of one parsed element, against the top-level input it was read from.
fn check_tlv(input: &[u8], t: &Tlv<'_>) {
    let enc = t.encoding();
    let at = offset_in(input, enc).expect("encoding is a sub-slice of the input");
    assert_eq!(at, t.offset(), "offset() is where the encoding lies");
    let hdr = t.header();
    assert!(hdr.len() >= 2, "an identifier and a length octet at least");
    assert_eq!(
        hdr.as_ptr(),
        enc.as_ptr(),
        "the header is the prefix of the encoding"
    );
    assert!(hdr.len() <= enc.len());
    let c = t.content();
    assert_eq!(
        c.as_ptr() as usize,
        enc.as_ptr() as usize + hdr.len(),
        "contents follow the header"
    );
    let eoc_len = if t.is_indefinite() { 2 } else { 0 };
    assert_eq!(
        hdr.len() + c.len() + eoc_len,
        enc.len(),
        "header + contents (+ EOC) is the encoding"
    );
    match t.length() {
        Length::Definite(n) => assert_eq!(
            n,
            c.len() as u64,
            "a definite length is the contents length"
        ),
        Length::Indefinite => {
            assert!(t.is_constructed());
            assert_eq!(
                &enc[enc.len() - 2..],
                &[0, 0],
                "an indefinite encoding ends with the EOC"
            );
        }
    }
    assert!(at + enc.len() <= input.len());
}

/// Walk the whole tree without recursion (the walk must not be the thing that overflows the stack). Returns
/// whether any fault was met and the contents of up to `want` nodes, spread over the tree.
fn walk<'a>(input: &'a [u8], root: &Tlv<'a>, rng: &mut Rng, want: usize) -> (bool, Vec<&'a [u8]>) {
    struct Frame<'a> {
        it: Children<'a>,
        next_at: usize,
        end: usize,
        faulted: bool,
    }
    let frame = |t: &Tlv<'a>| {
        let next_at = t.offset() + t.header().len();
        Frame {
            it: t.children(),
            next_at,
            // A primitive element has no children, whatever its contents.
            end: next_at
                + if t.is_constructed() {
                    t.content().len()
                } else {
                    0
                },
            faulted: false,
        }
    };
    let mut stack = vec![frame(root)];
    let mut faulted = false;
    let mut contents = vec![root.content()];
    let mut nodes = 0usize;
    while let Some(top) = stack.last_mut() {
        match top.it.next() {
            None => {
                if !top.faulted {
                    assert_eq!(
                        top.next_at, top.end,
                        "children fill the parent's contents exactly"
                    );
                }
                stack.pop();
            }
            Some(Ok(c)) => {
                check_tlv(input, &c);
                assert_eq!(
                    c.offset(),
                    top.next_at,
                    "children are contiguous and in order"
                );
                top.next_at += c.encoding().len();
                assert!(
                    top.next_at <= top.end,
                    "a child lies inside its parent's contents"
                );
                nodes += 1;
                assert!(
                    nodes <= input.len(),
                    "more nodes than octets: the walk is not bounded"
                );
                // Reservoir sample, so a large tree is sampled over its whole length.
                if contents.len() < want {
                    contents.push(c.content());
                } else if rng.below(nodes) < want {
                    let i = rng.below(want);
                    contents[i] = c.content();
                }
                if c.is_constructed() {
                    stack.push(frame(&c));
                }
            }
            Some(Err(e)) => {
                bounded(&e, input.len(), "children()");
                top.faulted = true;
                faulted = true;
                assert!(
                    top.it.next().is_none(),
                    "an iterator that returned an error ends"
                );
                stack.pop();
            }
        }
    }
    (faulted, contents)
}

const STRING_TYPES: [StringType; 11] = [
    StringType::Numeric,
    StringType::Printable,
    StringType::Visible,
    StringType::Ia5,
    StringType::Utf8,
    StringType::Bmp,
    StringType::Universal,
    StringType::Teletex,
    StringType::Videotex,
    StringType::Graphic,
    StringType::General,
];

const PRIMS: [Prim; 22] = [
    Prim::Boolean,
    Prim::Integer,
    Prim::Enumerated,
    Prim::BitString { named: false },
    Prim::BitString { named: true },
    Prim::OctetString,
    Prim::Null,
    Prim::Oid,
    Prim::RelOid,
    Prim::Real,
    Prim::Utc,
    Prim::Generalized,
    Prim::Date,
    Prim::TimeOfDay,
    Prim::DateTime,
    Prim::Duration,
    Prim::Time,
    Prim::OidIri,
    Prim::RelOidIri,
    Prim::Str(StringType::Utf8),
    Prim::Str(StringType::Printable),
    Prim::Str(StringType::Bmp),
];

/// Run the schema checks of the typed layer on `input`.
fn typed_battery(input: &[u8], mode: Mode, limits: &Limits) {
    let cx = Context::new(mode);
    let int = Schema::Prim(Prim::Integer);
    let any = Schema::Sequence(None);
    let fields = [
        Field {
            schema: int,
            presence: Presence::DefaultInt(0),
        },
        Field {
            schema: Schema::Prim(Prim::Boolean),
            presence: Presence::DefaultBool(false),
        },
        Field {
            schema: Schema::Prim(Prim::OctetString),
            presence: Presence::Optional,
        },
        Field {
            schema: Schema::Sequence(None),
            presence: Presence::Required,
        },
    ];
    let structured = [
        CERTIFICATE,
        any,
        Schema::SequenceOf(&any),
        Schema::SequenceOf(&int),
        Schema::SetOf(None),
        Schema::SetOf(Some(&int)),
        Schema::SetOf(Some(&any)),
        Schema::Set(Some(&fields)),
        Schema::Sequence(Some(&fields)),
        Schema::Tagged {
            class: berkit::tag::Class::ContextSpecific,
            number: 0,
            explicit: true,
            inner: &any,
        },
        Schema::Tagged {
            class: berkit::tag::Class::ContextSpecific,
            number: 1,
            explicit: false,
            inner: &Schema::Prim(Prim::OctetString),
        },
    ];
    for s in &structured {
        if let Err(typed::TypedError::Class(e)) = typed::validate(input, s, cx, limits) {
            bounded(&e, input.len(), "typed::validate");
        }
    }
    if let Ok((t, _)) = Tlv::parse(input, mode, limits) {
        for p in PRIMS {
            let s = Schema::Prim(p);
            if let Err(typed::TypedError::Class(e)) = typed::validate(input, &s, cx, limits) {
                bounded(&e, input.len(), "typed::validate prim");
            }
            if let Err(typed::TypedError::Class(e)) = typed::check_content(&t, &s, cx) {
                bounded(&e, input.len(), "typed::check_content");
            }
        }
        for seg in [3u64, 4, 12, 19, 23, 30] {
            let leaves = Leaves::new(&t, seg);
            let mut n = 0usize;
            for leaf in leaves.clone() {
                offset_in(input, leaf).expect("a leaf is a sub-slice of the input");
                n += 1;
                assert!(n <= input.len() + 1, "leaves are bounded by the input");
            }
            let _ = leaves.check();
            let mut run = leaves.clone();
            for _ in run.by_ref() {}
            if let Some(typed::TypedError::Class(e)) = run.fault() {
                bounded(&e, input.len(), "Leaves::fault");
            }
        }
    }
}

/// The content functions on one slice, in both modes.
fn content_battery(c: &[u8], rng: &mut Rng) {
    let len = c.len();
    for mode in [Mode::Ber, Mode::Der] {
        match integer::decode(c, mode) {
            Ok(i) => {
                let mut out = vec![0u8; i.as_bytes().len()];
                if let Some(n) = i.write_magnitude(&mut out) {
                    assert!(n <= out.len() && n >= 1);
                }
                assert!(i.write_magnitude(&mut []).is_none());
                let _ = (i.to_i128(), i.to_u128(), i.to_i64(), i.to_u64());
                let _ = (i.is_zero(), i.is_negative(), i.unsigned_bytes());
            }
            Err(e) => bounded(&e, len, "integer"),
        }
        match oid::ObjectIdentifier::decode(c, mode) {
            Ok(o) => arcs_battery(o.arcs(), len),
            Err(e) => bounded(&e, len, "oid"),
        }
        match oid::RelativeOid::decode(c, mode) {
            Ok(o) => {
                let mut n = 0;
                for a in o.arcs() {
                    arc_battery(&a);
                    n += 1;
                    assert!(n <= len + 1);
                }
            }
            Err(e) => bounded(&e, len, "relative oid"),
        }
        match oid::OidIri::decode(c, mode) {
            Ok(i) => assert!(i.arcs().count() <= len + 1),
            Err(e) => bounded(&e, len, "oid-iri"),
        }
        match oid::RelativeOidIri::decode(c, mode) {
            Ok(i) => assert!(i.arcs().count() <= len + 1),
            Err(e) => bounded(&e, len, "relative oid-iri"),
        }
        match real::decode(c, mode) {
            Ok(r) => {
                let _ = r.kind();
                let mut out = vec![0u8; len];
                assert_eq!(r.write(&mut out), Some(len));
            }
            Err(e) => bounded(&e, len, "real"),
        }
        for named in [false, true] {
            match bitstring::BitString::decode(c, mode, named) {
                Ok(b) => {
                    assert_eq!(b.bits().count(), b.bit_len());
                    assert!(b.bit(b.bit_len()).is_none());
                    assert!(b.bit_len() <= b.data().len() * 8);
                    assert!(b.unused_bits() <= 7);
                    let _ = bitstring::named_bit_list_len(b.data(), b.bit_len());
                    let _ = b.is_empty();
                }
                Err(e) => bounded(&e, len, "bitstring"),
            }
        }
        let _ = octetstring::decode(c);
        let _ = boolean::decode(c, mode).map_err(|e| bounded(&e, len, "boolean"));
        let _ = null::decode(c, mode).map_err(|e| bounded(&e, len, "null"));
        for st in STRING_TYPES {
            let _ = strings::validate(st, c).map_err(|e| bounded(&e, len, "strings"));
            // The same contents in two segments split anywhere must not panic or change the verdict.
            let cut = if len == 0 { 0 } else { rng.below(len + 1) };
            let mut v = strings::Validator::new(st);
            let split = v
                .feed(&c[..cut])
                .and_then(|()| v.feed(&c[cut..]))
                .and_then(|()| v.finish());
            assert_eq!(
                split.is_ok(),
                strings::validate(st, c).is_ok(),
                "{st:?} split at {cut}"
            );
            let _ = strings::validate_segments(st, [&c[..cut], &c[cut..]]);
        }
        let _ = strings::check_form(mode, true);
        for f in [
            time::parse_generalized_time(c, mode),
            time::parse_utc_time(c, mode, 50),
            time::parse_utc_time(c, mode, 0),
            time::parse_utc_time(c, mode, 99),
        ] {
            match f {
                Ok(f) => {
                    let _ = f.is_calendar_date();
                    let _ = f.zone.minutes_east();
                }
                Err(e) => bounded(&e, len, "time"),
            }
        }
        let _ = time::validate_duration(c, mode).map_err(|e| bounded(&e, len, "duration"));
        let _ = time::validate_time(c, mode).map_err(|e| bounded(&e, len, "time"));
    }
    let _ = time::parse_rfc5280_utc_time(c).map_err(|e| bounded(&e, len, "rfc5280 utc"));
    let _ = time::parse_rfc5280_generalized_time(c).map_err(|e| bounded(&e, len, "rfc5280 gen"));
    let _ = time::parse_date(c).map_err(|e| bounded(&e, len, "date"));
    let _ = time::parse_time_of_day(c).map_err(|e| bounded(&e, len, "time of day"));
    let _ = time::parse_date_time(c).map_err(|e| bounded(&e, len, "date-time"));

    // Constructed strings: the contents cut into segments of random length.
    let mut segs: Vec<&[u8]> = Vec::new();
    let mut at = 0;
    while at < len {
        let n = 1 + rng.below(len - at);
        let n = n.min(1 + rng.below(8));
        segs.push(&c[at..at + n]);
        at += n;
    }
    for mode in [Mode::Ber, Mode::Der] {
        match bitstring::ConstructedBitString::new(segs.iter().copied(), mode) {
            Ok(b) => {
                assert_eq!(b.data_octets().count(), b.octet_len());
                assert_eq!(b.bits().count(), b.bit_len());
                let mut out = vec![0u8; b.octet_len()];
                let _ = b.copy_data(&mut out);
                let _ = b.copy_data(&mut []);
            }
            Err(e) => bounded(&e, len, "constructed bitstring"),
        }
        match octetstring::ConstructedOctetString::new(segs.iter().copied(), mode) {
            Ok(o) => {
                assert_eq!(o.len(), len);
                assert_eq!(o.octets().count(), len);
                let mut out = vec![0u8; len];
                assert_eq!(o.copy_into(&mut out), Ok(len));
                if len > 0 {
                    assert!(o.copy_into(&mut out[..len - 1]).is_err());
                }
            }
            Err(e) => bounded(&e, len, "constructed octetstring"),
        }
    }
}

fn arc_battery(a: &oid::Arc<'_>) {
    let _ = (a.to_u128(), a.to_u64());
    let mut out = vec![0u8; a.octets().len()];
    if let Some(n) = a.be_bytes(&mut out) {
        assert!(n >= 1 && n <= out.len());
    }
    assert!(a.be_bytes(&mut []).is_none());
}

fn arcs_battery(arcs: oid::ObjectIdentifierArcs<'_>, len: usize) {
    let mut n = 0;
    for a in arcs {
        arc_battery(&a);
        n += 1;
        assert!(n <= len + 2, "arcs are bounded by the contents");
    }
}

/// Everything, for one input. Run on the worker thread.
fn exercise(input: &[u8]) {
    let mut rng = Rng::new(input.len() as u64 ^ 0xA5A5_5A5A);
    // Header decoders on the raw octets.
    if let Ok((tag, n)) = Tag::decode(input) {
        assert!(n >= 1 && n <= input.len());
        let mut buf = [0u8; 16];
        let _ = tag.encode(&mut buf);
    } else if let Err(e) = Tag::decode(input) {
        bounded(&e, input.len(), "Tag::decode");
    }
    for mode in [Mode::Ber, Mode::Der] {
        match length::decode(input, mode) {
            Ok((_, n)) => assert!(n >= 1 && n <= input.len()),
            Err(e) => bounded(&e, input.len(), "length::decode"),
        }
    }
    content_battery(input, &mut rng);

    for mode in [Mode::Ber, Mode::Der] {
        for limits in &LIMITS {
            match Tlv::parse(input, mode, limits) {
                Ok((t, rest)) => {
                    check_tlv(input, &t);
                    assert_eq!(t.offset(), 0);
                    assert_eq!(t.encoding().len() + rest.len(), input.len());
                    assert_eq!(
                        rest.as_ptr() as usize,
                        input.as_ptr() as usize + t.encoding().len()
                    );
                    let valid = t.validate();
                    if let Err(e) = &valid {
                        bounded(e, input.len(), "validate");
                    }
                    let (faulted, contents) = walk(input, &t, &mut rng, 10);
                    assert_eq!(
                        valid.is_ok(),
                        !faulted,
                        "validate and a walk of children() agree"
                    );
                    match Tlv::parse_exact(input, mode, limits) {
                        Ok(x) => {
                            assert!(valid.is_ok() && rest.is_empty());
                            assert_eq!(x.encoding(), input);
                            check_tlv(input, &x);
                        }
                        Err(e) => bounded(&e, input.len(), "parse_exact"),
                    }
                    for c in contents {
                        content_battery(c, &mut rng);
                    }
                }
                Err(e) => {
                    bounded(&e, input.len(), "parse");
                    if let Err(e2) = Tlv::parse_exact(input, mode, limits) {
                        bounded(&e2, input.len(), "parse_exact");
                    } else {
                        panic!("parse_exact succeeded where parse failed with {e}");
                    }
                }
            }
        }
        typed_battery(input, mode, &Limits::DEFAULT);
    }
    // The permissive limits are the ones that reach HARD_MAX_DEPTH.
    typed_battery(input, Mode::Ber, &LIMITS[2]);
}

fn sweep(label: &str, bases: &[Vec<u8>], seed: u64, per_base: usize) -> usize {
    let mut run = Runner::new(WATCHDOG, exercise);
    let mut rng = Rng::new(seed);
    let mut count = 0;
    for base in bases {
        run.run(&format!("{label} base"), base);
        count += 1;
        for _ in 0..per_base {
            let m = mutate(base, &mut rng);
            run.run(label, &m);
            count += 1;
        }
    }
    count
}

// --------------------------------------------------------------------------------------------- tests

#[test]
fn mutated_real_certificates_hold_the_invariants() {
    let certs = real_certificates();
    assert_eq!(certs.len(), 40);
    let start = Instant::now();
    let n = sweep("certificate", &certs, 0x0123_4567_89AB_CDEF, 20);
    eprintln!("certificates: {n} inputs in {:?}", start.elapsed());
    assert_eq!(n, 40 * 21);
}

#[test]
fn mutated_corpus_inputs_hold_the_invariants() {
    let inputs = corpus_inputs();
    assert!(
        inputs.len() > 800,
        "the corpus has {} hex inputs",
        inputs.len()
    );
    let start = Instant::now();
    let n = sweep("corpus", &inputs, 0xFEED_FACE_CAFE_F00D, 3);
    eprintln!("corpus: {n} inputs in {:?}", start.elapsed());
}

#[test]
fn crafted_length_fields_hold_the_invariants() {
    let mut run = Runner::new(WATCHDOG, exercise);
    let inputs = crafted_length_inputs();
    for input in &inputs {
        run.run("crafted length", input);
    }
    eprintln!("crafted length fields: {} inputs", inputs.len());
}

#[test]
fn crafted_nesting_shapes_hold_the_invariants() {
    let shapes = nesting_shapes();
    let mut run = Runner::new(WATCHDOG, exercise);
    let mut rng = Rng::new(0x5EED_0000_0000_0001);
    let start = Instant::now();
    for s in &shapes {
        run.run("nesting", s);
        // A broken copy of each: the nesting is the place a bound is most easily lost.
        let m = mutate(s, &mut rng);
        run.run("nesting mutated", &m);
    }
    eprintln!(
        "nesting shapes: {} inputs in {:?}",
        shapes.len() * 2,
        start.elapsed()
    );
}

#[test]
fn random_bytes_hold_the_invariants() {
    let mut run = Runner::new(WATCHDOG, exercise);
    let inputs = random_inputs(4000, 0xDEAD_BEEF_0BAD_F00D);
    for input in &inputs {
        run.run("random", input);
    }
    eprintln!("random: {} inputs", inputs.len());
}

// ------------------------------------------------------------------------------------------ watchdog

fn spin(_input: &[u8]) {
    loop {
        thread::sleep(Duration::from_millis(5));
    }
}

fn explode(_input: &[u8]) {
    panic!("deliberate panic for the harness check");
}

#[test]
#[should_panic(expected = "HANG")]
fn the_watchdog_turns_a_hang_into_a_failure() {
    let mut run = Runner::new(Duration::from_millis(300), spin);
    run.run("harness", &[0x30, 0x00]);
}

#[test]
#[should_panic(expected = "deliberate panic for the harness check")]
fn the_runner_turns_a_panic_into_a_failure_naming_the_input() {
    let mut run = Runner::new(Duration::from_millis(300), explode);
    run.run("harness", &[0x30, 0x00]);
}

// ------------------------------------------------------------------------------------ nesting and stack

/// Check one limit on every shape of nesting: `levels` counts the INTEGER inside, so `depth` constructed
/// levels hold `depth + 1`. Up to the bound the input is accepted (or, with no EOC, `missing_eoc`); beyond it
/// `limit_exceeded`, never a panic or a stack overflow. In DER the indefinite shapes are refused at the first
/// header.
type Shape = (&'static str, fn(usize) -> Vec<u8>, bool, bool);

fn check_depths(limits: &Limits, depths: &[usize]) {
    let int = [0x02, 0x01, 0x05];
    let bound = limits.max_depth.min(Limits::HARD_MAX_DEPTH);
    let shapes: [Shape; 4] = [
        (
            "definite",
            |d| nest_definite(0x30, d, &[0x02, 0x01, 0x05]),
            false,
            false,
        ),
        (
            "indefinite",
            |d| nest_indefinite(0x30, d, &[0x02, 0x01, 0x05], d),
            true,
            false,
        ),
        (
            "indefinite A0 around a BIT STRING",
            |d| nest_indefinite(0xA0, d, &[0x03, 0x02, 0x00, 0x41], d),
            true,
            false,
        ),
        (
            "indefinite without EOC",
            |d| nest_indefinite(0x30, d, &[0x02, 0x01, 0x05], 0),
            true,
            true,
        ),
    ];
    let _ = int;
    for (name, make, indefinite, no_eoc) in shapes {
        for mode in [Mode::Ber, Mode::Der] {
            for &d in depths {
                let input = make(d);
                let r = Tlv::parse_exact(&input, mode, limits);
                let what = format!("{name}, {mode:?}, {d} constructed levels, limits {limits:?}");
                if mode.is_der() && indefinite {
                    assert_eq!(r.unwrap_err().class, ErrorClass::IndefiniteLength, "{what}");
                } else if d < bound {
                    if no_eoc {
                        assert_eq!(r.unwrap_err().class, ErrorClass::MissingEoc, "{what}");
                    } else {
                        r.unwrap_or_else(|e| panic!("{what}: {e}"));
                    }
                } else {
                    let e = r.expect_err(&what);
                    assert_eq!(e.class, ErrorClass::LimitExceeded, "{what}");
                    assert!(e.offset <= input.len(), "{what}: offset {}", e.offset);
                }
                // The other entry points must also return, whatever they answer.
                if let Ok((t, _)) = Tlv::parse(&input, mode, limits) {
                    let _ = t.validate();
                    let _ = walk(&input, &t, &mut Rng::new(1), 4);
                    let _ = Leaves::new(&t, 4).check();
                }
                let _ =
                    typed::validate(&input, &Schema::Sequence(None), Context::new(mode), limits);
            }
        }
    }
}

/// How long the child process may run: it takes well under a second.
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

const CHILD_ENV: &str = "BERKIT_ROBUSTNESS_STACK_CHILD";

fn on_stack(size: usize, f: fn()) {
    thread::Builder::new()
        .stack_size(size)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the checks on this stack passed");
}

/// Only does anything in the child process started by the test below.
#[test]
fn stack_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    // 256 KiB hold the default limit of 64 levels, and a refusal at 65 or far beyond, in a debug build.
    on_stack(SMALL_STACK, || {
        // The input the recursive reader overflowed a 256 KiB debug stack on (123 levels): `a0 80` x 123, the
        // BIT STRING `03 02 00 41`, `00 00` x 123, with no depth limit asked for.
        let unlimited = Limits {
            max_depth: usize::MAX,
            max_content_length: u64::MAX,
        };
        let old_input = nest_indefinite(0xA0, 123, &[0x03, 0x02, 0x00, 0x41], 123);
        assert_eq!(old_input.len(), 123 * 4 + 4);
        let t = Tlv::parse_exact(&old_input, Mode::Ber, &unlimited).expect("accepted");
        assert!(t.validate().is_ok());
        check_depths(
            &Limits::DEFAULT,
            &[1, 2, 62, 63, 64, 65, 100, 513, 4000, 100_000],
        );
        let tight = Limits {
            max_depth: 4,
            max_content_length: 1 << 20,
        };
        check_depths(&tight, &[1, 2, 3, 4, 5, 6, 1000]);
    });
    // The hard bound holds on the same 256 KiB stack when the caller asks for no limit at all: 512 levels are
    // accepted, 513 are `limit_exceeded`, and nothing deeper costs more stack.
    on_stack(SMALL_STACK, || {
        let unlimited = Limits {
            max_depth: usize::MAX,
            max_content_length: u64::MAX,
        };
        let h = Limits::HARD_MAX_DEPTH;
        check_depths(
            &unlimited,
            &[h - 2, h - 1, h, h + 1, 2 * h, 10_000, 100_000],
        );
        // The deepest input accepted: `h - 1` constructed levels around the INTEGER, h levels in all.
        let deepest = nest_indefinite(0xA0, h - 1, &[0x03, 0x02, 0x00, 0x41], h - 1);
        let t = Tlv::parse_exact(&deepest, Mode::Ber, &unlimited).expect("512 levels are accepted");
        assert!(t.validate().is_ok());
        let one_more = nest_indefinite(0xA0, h, &[0x03, 0x02, 0x00, 0x41], h);
        let e = Tlv::parse_exact(&one_more, Mode::Ber, &unlimited).unwrap_err();
        assert_eq!(e.class, ErrorClass::LimitExceeded);
        let definite = nest_definite(0xA0, h, &[0x03, 0x02, 0x00, 0x41]);
        let t = Tlv::parse_exact(&definite, Mode::Ber, &unlimited);
        assert_eq!(t.unwrap_err().class, ErrorClass::LimitExceeded);
    });
}

/// Nesting beyond `max_depth` is `limit_exceeded`, and the stack the reader needs does not grow with the depth:
/// the default limit (64 levels) and `Limits::HARD_MAX_DEPTH` (512 levels, with no limit asked for) both run on
/// a 256 KiB stack, in a debug build. A stack overflow aborts the whole process, so the checks run in a child
/// process (this test binary again) and the parent reports its exit status as a failure of this test.
///
/// History: the reader used to recurse once per level, about 2.7 KiB a level in a debug build (512 levels of
/// indefinite length needed between 1.3 and 1.4 MiB, 123 levels of `a0 80` overflowed 256 KiB) and about 0.4
/// KiB in a release build. It now walks the tree with an explicit stack of `HARD_MAX_DEPTH` small frames
/// (`src/tlv.rs`, `walk`), and this test is what fails if a recursion per level comes back.
#[test]
fn nesting_beyond_max_depth_is_refused_and_fits_a_small_stack() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let me = std::env::current_exe().expect("test binary");
    let mut child = Command::new(me)
        .args(["--exact", "stack_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run the child");
    let mut readers = Vec::new();
    let out = child.stdout.take().expect("piped");
    let err = child.stderr.take().expect("piped");
    for mut pipe in [Box::new(out) as Box<dyn Read + Send>, Box::new(err)] {
        readers.push(thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        }));
    }
    // The watchdog again: a child that loops forever is a failure, not a hung suite.
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break Some(status);
        }
        if start.elapsed() > CHILD_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let text: String = readers
        .into_iter()
        .map(|r| r.join().unwrap_or_default())
        .collect();
    let status = status.unwrap_or_else(|| {
        panic!(
            "HANG: the child ran past {CHILD_TIMEOUT:?}:
{text}"
        )
    });
    assert!(
        status.success(),
        "the child died or failed ({status:?}):
{text}"
    );
    assert!(
        text.contains("1 passed"),
        "the child did not run the check:
{text}"
    );
}
