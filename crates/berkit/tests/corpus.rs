//! Runs the conformance corpus (`corpus/`) against the crate.
//!
//! Every case runs once per mode it names. The test passes when the set of disagreeing case ids equals
//! `tests/corpus_known.md` exactly: a new disagreement and a case that starts agreeing both fail it.
//! The sha256 of each corpus file is checked against the manifest first.

mod common;

use berkit::Mode;
use berkit::tlv::Limits;
use common::json::Json;
use common::model::{self, Case, Corpus, Input, Node, Op, Outcome};
use common::typed::{self, Decoded, Failure};
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

/// JSON text for failure messages.
fn json_text(j: &Json) -> String {
    match j {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => n.clone(),
        Json::Str(s) => format!("{s:?}"),
        Json::Arr(a) => format!(
            "[{}]",
            a.iter().map(json_text).collect::<Vec<_>>().join(",")
        ),
        Json::Obj(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{k:?}:{}", json_text(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// JSON equality with the members of an object in any order.
fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Arr(x), Json::Arr(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Json::Obj(x), Json::Obj(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && json_eq(v, v2)))
        }
        _ => a == b,
    }
}

fn show(r: &Result<Decoded, Failure>) -> String {
    match r {
        Ok(d) => match &d.value {
            Some(v) => format!("tree {} value {}", d.node.describe(), json_text(v)),
            None => format!("tree {} (no value form)", d.node.describe()),
        },
        Err(Failure::Class(e)) => format!("error {}", e.class.name()),
        Err(Failure::Unsupported(w)) => format!("adapter: {w}"),
        Err(Failure::Accessor(w)) => format!("accessor: {w}"),
    }
}

/// Does a decode result match the expected outcome?
fn decode_matches(got: &Result<Decoded, Failure>, want: &Outcome) -> bool {
    match (got, want) {
        (Ok(d), Outcome::Tree(w)) => &d.node == w,
        (Ok(d), Outcome::Value { tree, value }) => {
            d.value.as_ref().is_some_and(|v| json_eq(v, value))
                && tree.as_ref().is_none_or(|t| t == &d.node)
        }
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
        Outcome::Value { tree, value } => match tree {
            Some(t) => format!("tree {} value {}", t.describe(), json_text(value)),
            None => format!("value {}", json_text(value)),
        },
    }
}

/// Run one case in one mode; `Err` describes the disagreement.
fn run_one(case: &Case, mode: Mode, want: &Outcome, limits: &Limits) -> Result<(), String> {
    let ty = case.ty.as_deref();
    let disagree = |got: String| format!("expected {}, got {got}", want_text(want));
    match (case.op, &case.input) {
        (Op::Decode, Input::Hex(h)) => {
            let got = typed::decode(h, ty, mode, limits, case.pivot);
            if decode_matches(&got, want) {
                Ok(())
            } else {
                Err(disagree(show(&got)))
            }
        }
        (Op::Roundtrip, Input::Hex(h)) => {
            let got = typed::decode(h, ty, mode, limits, case.pivot);
            if !decode_matches(&got, want) {
                return Err(disagree(show(&got)));
            }
            let Ok(Decoded { node: tree, value }) = got else {
                return Ok(());
            };
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
                match typed::decode(&again, ty, mode, limits, case.pivot) {
                    Ok(d2)
                        if d2.node.same_ignoring_length(&tree)
                            && match (&d2.value, &value) {
                                (Some(a), Some(b)) => json_eq(a, b),
                                (None, None) => true,
                                _ => false,
                            } => {}
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
                        Failure::Accessor(w) => disagree(format!("accessor: {w}")),
                    })?,
                    Some(ty.as_str()),
                ),
                Input::Hex(_) => return Err("encode case with a hex input".into()),
            };
            if &bytes != h {
                return Err(disagree(format!("hex {}", model::hex(&bytes))));
            }
            // The output must decode back to the input tree (`format.md`, Operations).
            let back = typed::decode(&bytes, back_ty, mode, limits, None);
            match (&back, input) {
                (Ok(d), Input::Tree(n)) if &d.node == n => Ok(()),
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
            // A panic inside the crate is a disagreement of that case, not the end of the run: the other cases
            // still report.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_one(case, *mode, want, &limits)
            }))
            .unwrap_or_else(|p| {
                let what = p
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                Err(format!("the crate panicked: {what}"))
            });
            match outcome {
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
    assert_eq!(corpus.hashes.len(), 6);
    for (file, want, got) in &corpus.hashes {
        assert_eq!(want, got, "{file}: sha256 differs from corpus.json");
    }
}

#[test]
fn corpus_agrees_with_the_code_except_the_known_list() {
    let corpus = model::load().expect("the corpus loads");
    assert_eq!(
        corpus.cases.len(),
        1179,
        "the runner was written for 1179 cases"
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

/// A one-mode DER decode case for the runner self-tests.
fn mk_case(ty: Option<&str>, hex: &str, want: Outcome) -> Case {
    Case {
        id: "t".into(),
        op: Op::Decode,
        ty: ty.map(str::to_string),
        input: Input::Hex(model::unhex(hex).unwrap()),
        expect: vec![(Mode::Der, want)],
        policy: None,
        pivot: None,
    }
}

#[test]
fn a_wrong_value_is_reported() {
    let limits = Limits::DEFAULT;
    let mk = mk_case;
    let der = |c: &Case| run_one(c, Mode::Der, &c.expect[0].1, &limits);
    // A value: INTEGER 02 02 00 ff is 255, and a case that says 256 or a different shape is reported.
    let value = |ty: &str, hex: &str, v: Json| {
        let c = mk(
            Some(ty),
            hex,
            Outcome::Value {
                tree: None,
                value: v,
            },
        );
        der(&c)
    };
    assert!(value("INTEGER", "020200ff", Json::Str("255".into())).is_ok());
    assert!(value("INTEGER", "020200ff", Json::Str("256".into())).is_err());
    assert!(value("INTEGER", "020200ff", Json::Num("255".into())).is_err());
    assert!(
        value(
            "OBJECT IDENTIFIER",
            "0603883703",
            Json::Str("2.999.3".into())
        )
        .is_ok()
    );
    assert!(
        value(
            "OBJECT IDENTIFIER",
            "0603883703",
            Json::Str("2.100.3".into())
        )
        .is_err()
    );
    // A type with no value form cannot satisfy a value outcome.
    assert!(value("DATE", "1f1f0419920722", Json::Str("19920722".into())).is_err());
    // An object's members may come in any order.
    let two = |a: &str| {
        Json::Obj(vec![
            ("unused".into(), Json::Num("0".into())),
            ("hex".into(), Json::Str(a.into())),
        ])
    };
    let swapped = Json::Obj(vec![
        ("hex".into(), Json::Str("ff".into())),
        ("unused".into(), Json::Num("0".into())),
    ]);
    assert!(json_eq(&two("ff"), &swapped));
    assert!(!json_eq(&two("fe"), &swapped));
    // A tree beside a value: both must hold.
    let t = Node::from_json(
        &common::json::parse(
            r#"{"class":"universal","constructed":false,"tag":1,"length":{"form":"short"},"content":"ff"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let both = |tree: Node, v: bool| {
        der(&mk(
            Some("BOOLEAN"),
            "0101ff",
            Outcome::Value {
                tree: Some(tree),
                value: Json::Bool(v),
            },
        ))
    };
    assert!(both(t.clone(), true).is_ok());
    assert!(both(t.clone(), false).is_err());
    let mut other_tree = t;
    other_tree.tag = 2;
    assert!(both(other_tree, true).is_err());
}

#[test]
fn a_wrong_expectation_is_reported() {
    // Guard against a runner that cannot fail: a hand-built case whose expectation is wrong in each way.
    let limits = Limits::DEFAULT;
    let der = |c: &Case| run_one(c, Mode::Der, &c.expect[0].1, &limits);
    let mk = mk_case;
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
