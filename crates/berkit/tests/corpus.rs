//! Runs the conformance corpus (`corpus/`) against the crate.
//!
//! Every case runs once per mode it names. The test passes when the set of disagreeing case ids equals
//! `tests/corpus_known.md` exactly: a new disagreement and a case that starts agreeing both fail it.
//! The sha256 of each corpus file is checked against the manifest first.

mod common;

use berkit::Mode;
use berkit::tlv::Limits;
use common::model::{self, Case, Corpus, Input, Node, Op, Outcome};
use common::typed::{self, Failure};
use std::collections::{BTreeMap, BTreeSet};

fn mode_name(m: Mode) -> &'static str {
    if m.is_der() { "der" } else { "ber" }
}

fn limits_of(c: &Corpus) -> Limits {
    Limits {
        max_depth: c.limits.depth,
        max_content_length: c.limits.length,
    }
}

fn show(r: &Result<Node, Failure>) -> String {
    match r {
        Ok(n) => format!("tree {}", n.describe()),
        Err(Failure::Class(e)) => format!("error {}", e.class.name()),
        Err(Failure::Unsupported(w)) => format!("adapter: {w}"),
    }
}

/// Does a decode result match the expected outcome?
fn decode_matches(got: &Result<Node, Failure>, want: &Outcome) -> bool {
    match (got, want) {
        (Ok(n), Outcome::Tree(w)) => n == w,
        // `error_offset` is advisory and not compared.
        (Err(Failure::Class(e)), Outcome::Error(name)) => e.class.name() == name,
        _ => false,
    }
}

fn want_text(want: &Outcome) -> String {
    match want {
        Outcome::Tree(n) => format!("tree {}", n.describe()),
        Outcome::Hex(h) => format!("hex {}", model::hex(h)),
        Outcome::Error(e) => format!("error {e}"),
    }
}

/// Run one case in one mode; `Err` describes the disagreement.
fn run_one(case: &Case, mode: Mode, want: &Outcome, limits: &Limits) -> Result<(), String> {
    let ty = case.ty.as_deref();
    let disagree = |got: String| format!("expected {}, got {got}", want_text(want));
    match (case.op, &case.input) {
        (Op::Decode, Input::Hex(h)) => {
            let got = typed::decode(h, ty, mode, limits);
            if decode_matches(&got, want) {
                Ok(())
            } else {
                Err(disagree(show(&got)))
            }
        }
        (Op::Roundtrip, Input::Hex(h)) => {
            let got = typed::decode(h, ty, mode, limits);
            if !decode_matches(&got, want) {
                return Err(disagree(show(&got)));
            }
            let Ok(tree) = got else { return Ok(()) };
            let mut again = Vec::new();
            tree.encode(false, &mut again)
                .map_err(|e| format!("re-encoding the decoded tree: {e}"))?;
            if mode.is_der() {
                // In DER the re-encoded bytes equal the input.
                if &again != h {
                    return Err(format!(
                        "DER re-encoding {} differs from input {}",
                        model::hex(&again),
                        model::hex(h)
                    ));
                }
            } else {
                // In BER the bytes may differ, but they must decode to the same tree but for the length.
                match typed::decode(&again, ty, mode, limits) {
                    Ok(t2) if t2.same_ignoring_length(&tree) => {}
                    other => {
                        return Err(format!(
                            "BER re-encoding {} decodes to {}",
                            model::hex(&again),
                            show(&other)
                        ));
                    }
                }
            }
            Ok(())
        }
        (Op::Encode, input) => {
            let Outcome::Hex(h) = want else {
                return Err("encode case without a hex outcome".into());
            };
            let (bytes, back_ty) = match input {
                Input::Tree(n) => {
                    let mut out = Vec::new();
                    n.encode(true, &mut out).map_err(disagree)?;
                    (out, ty)
                }
                Input::Typed { ty, value } => (
                    typed::encode_typed(ty, value).map_err(|f| match f {
                        Failure::Unsupported(w) => disagree(format!("adapter: {w}")),
                        Failure::Class(e) => disagree(format!("error {}", e.class.name())),
                    })?,
                    Some(ty.as_str()),
                ),
                Input::Hex(_) => return Err("encode case with a hex input".into()),
            };
            if &bytes != h {
                return Err(disagree(format!("hex {}", model::hex(&bytes))));
            }
            // The output must decode back to the input tree (`format.md`, Operations).
            let back = typed::decode(&bytes, back_ty, mode, limits);
            match (&back, input) {
                (Ok(t), Input::Tree(n)) if t == n => Ok(()),
                (Ok(_), Input::Typed { .. }) => Ok(()),
                _ => Err(format!(
                    "output {} does not decode back: {}",
                    model::hex(&bytes),
                    show(&back)
                )),
            }
        }
        _ => Err("operation and input do not fit together".into()),
    }
}

#[derive(Default)]
struct Report {
    /// (operation, mode) -> (run, agreed)
    counts: BTreeMap<(&'static str, &'static str), (usize, usize)>,
    /// case id -> one message per disagreeing mode
    disagreements: BTreeMap<String, Vec<String>>,
    typed: usize,
    policy: usize,
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Decode => "decode",
        Op::Encode => "encode",
        Op::Roundtrip => "roundtrip",
    }
}

fn run_all(corpus: &Corpus) -> Report {
    let limits = limits_of(corpus);
    let mut report = Report::default();
    for case in &corpus.cases {
        report.typed += usize::from(case.ty.is_some());
        report.policy += usize::from(case.policy.is_some());
        for (mode, want) in &case.expect {
            let slot = report
                .counts
                .entry((op_name(case.op), mode_name(*mode)))
                .or_default();
            slot.0 += 1;
            match run_one(case, *mode, want, &limits) {
                Ok(()) => slot.1 += 1,
                Err(msg) => report
                    .disagreements
                    .entry(case.id.clone())
                    .or_default()
                    .push(format!("{}: {msg}", mode_name(*mode))),
            }
        }
    }
    report
}

/// The ids in `corpus_known.md`, each with its classification.
fn known() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in include_str!("corpus_known.md").lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 4 || !line.starts_with('|') {
            continue;
        }
        let id = cells[1].trim_matches('`');
        if id == "Case id" || id.starts_with('-') {
            continue;
        }
        assert!(
            [
                "CODE-BUG",
                "CORPUS-BUG",
                "POLICY-GAP",
                "FORMAT-GAP",
                "ADAPTER-LIMIT"
            ]
            .contains(&cells[2]),
            "corpus_known.md: {id} has no valid classification"
        );
        assert!(!cells[3].is_empty(), "corpus_known.md: {id} has no reason");
        assert!(
            out.insert(id.to_string(), cells[2].to_string()).is_none(),
            "corpus_known.md: {id} listed twice"
        );
    }
    out
}

#[test]
fn corpus_files_match_the_manifest_sha256() {
    let corpus = model::load().expect("the corpus loads");
    assert_eq!(corpus.hashes.len(), 5);
    for (file, want, got) in &corpus.hashes {
        assert_eq!(want, got, "{file}: sha256 differs from corpus.json");
    }
}

#[test]
fn corpus_agrees_with_the_code_except_the_known_list() {
    let corpus = model::load().expect("the corpus loads");
    assert_eq!(
        corpus.cases.len(),
        931,
        "the runner was written for 931 cases"
    );
    assert_eq!(corpus.limits.depth, 64);
    assert_eq!(
        corpus.limits.tag_number,
        u64::MAX,
        "the crate's tag limit is u64::MAX"
    );
    let report = run_all(&corpus);

    let (mut run, mut ok) = (0, 0);
    for ((op, mode), (r, a)) in &report.counts {
        println!("{op:9} {mode}: run {r:4} agree {a:4}");
        run += r;
        ok += a;
    }
    println!(
        "corpus {}: {} cases, {run} runs, {ok} agree, {} cases disagree; {} typed, {} with a policy",
        corpus.version,
        corpus.cases.len(),
        report.disagreements.len(),
        report.typed,
        report.policy
    );

    let known = known();
    let actual: BTreeSet<&String> = report.disagreements.keys().collect();
    let expected: BTreeSet<&String> = known.keys().collect();
    let mut problems = Vec::new();
    for id in actual.difference(&expected) {
        problems.push(format!(
            "NEW disagreement {id}: {}",
            report.disagreements[*id].join(" | ")
        ));
    }
    for id in expected.difference(&actual) {
        problems.push(format!(
            "{id} is in corpus_known.md ({}) but now agrees",
            known[*id]
        ));
    }
    assert!(problems.is_empty(), "\n{}\n", problems.join("\n"));
}

#[test]
fn a_wrong_expectation_is_reported() {
    // Guard against a runner that cannot fail: a hand-built case whose expectation is wrong in each way.
    let limits = Limits::DEFAULT;
    let mk = |ty: Option<&str>, hex: &str, want: Outcome| Case {
        id: "t".into(),
        op: Op::Decode,
        ty: ty.map(str::to_string),
        input: Input::Hex(model::unhex(hex).unwrap()),
        expect: vec![(Mode::Der, want)],
        policy: None,
    };
    let der = |c: &Case| run_one(c, Mode::Der, &c.expect[0].1, &limits);
    // Right: DER refuses a BOOLEAN of 01.
    assert!(
        der(&mk(
            Some("BOOLEAN"),
            "010101",
            Outcome::Error("boolean_not_ff".into())
        ))
        .is_ok()
    );
    // Wrong class.
    assert!(
        der(&mk(
            Some("BOOLEAN"),
            "010101",
            Outcome::Error("bad_contents".into())
        ))
        .is_err()
    );
    // An error where the case says tree.
    let tree = Node::from_json(
        &common::json::parse(
            r#"{"class":"universal","constructed":false,"tag":1,"length":{"form":"short"},"content":"01"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(der(&mk(Some("BOOLEAN"), "010101", Outcome::Tree(tree.clone()))).is_err());
    // A tree that differs in one content octet.
    let mut other = tree;
    other.body = model::Body::Content(vec![2]);
    assert!(der(&mk(None, "010101", Outcome::Tree(other))).is_err());
}
