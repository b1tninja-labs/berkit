//! Real DER: 40 public CA certificates that this workspace did not produce, read by berkit and by an independent
//! reference walker (`tests/data/real/refdump.py`, standard library Python written without berkit or asn1core).
//!
//! 1. Every certificate must be accepted by `Tlv::parse_exact` in DER mode and by `typed::validate` against a
//!    hand-written X.509 Certificate schema (RFC 5280 section 4.1.1 and 4.1.2).
//! 2. The tree berkit reads must equal the reference's dump line for line: depth, class, number, form, length
//!    and header size of every node (`NN.tree`).
//! 3. Each certificate is mutated in ways X.690 makes errors. The reference predicts the class for each mutation
//!    and records the sha256 of the mutated bytes (`mutations.expected`); this file builds the mutations on its
//!    own, checks that it built the same bytes, and compares berkit's verdict with the prediction. Disagreements
//!    are listed in `KNOWN` with the reason; the test fails on a new disagreement and on a listed one that stops.

#[path = "common/cert_schema.rs"]
mod cert_schema;
#[path = "common/sha256.rs"]
mod sha256;

use berkit::Mode;
use berkit::length::Length;
use berkit::tag::Class;
use berkit::tlv::{Limits, Tlv};
use berkit::typed::{self, Context, Prim, Schema, TypedError};
use cert_schema::CERTIFICATE;
use sha256::sha256_hex;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/real")
}

fn certs() -> Vec<(String, Vec<u8>)> {
    let mut names: Vec<String> = fs::read_dir(data_dir())
        .expect("data/real")
        .filter_map(|e| {
            let n = e.ok()?.file_name().into_string().ok()?;
            (PathBuf::from(&n).extension().is_some_and(|x| x == "der")).then_some(n)
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| {
            let b = fs::read(data_dir().join(&n)).expect("read der");
            (n, b)
        })
        .collect()
}

fn verdict(der: &[u8], mode: Mode) -> Result<(), String> {
    typed::validate(der, &CERTIFICATE, Context::new(mode), &Limits::DEFAULT).map_err(|e| match e {
        TypedError::Class(e) => e.class.name().to_string(),
        TypedError::Mismatch(m) => format!("mismatch ({m})"),
    })
}

/// The pre-order list of nodes with their depth.
fn nodes<'a>(t: Tlv<'a>, depth: usize, out: &mut Vec<(usize, Tlv<'a>)>) {
    out.push((depth, t));
    for c in t.children() {
        nodes(c.expect("children of a validated tree"), depth + 1, out);
    }
}

fn dump_line(depth: usize, t: &Tlv<'_>) -> String {
    let class = match t.tag().class {
        Class::Universal => 'U',
        Class::Application => 'A',
        Class::ContextSpecific => 'C',
        Class::Private => 'P',
    };
    let Length::Definite(len) = t.length() else {
        panic!("indefinite length in a DER certificate")
    };
    let form = if t.is_constructed() { 'C' } else { 'P' };
    format!(
        "{depth} {class} {} {form} {len} {}",
        t.tag().number,
        t.header().len()
    )
}

#[test]
fn there_are_forty_certificates() {
    assert_eq!(certs().len(), 40);
}

#[test]
fn every_certificate_is_accepted_in_der() {
    for (name, der) in certs() {
        Tlv::parse_exact(&der, Mode::Der, &Limits::DEFAULT)
            .unwrap_or_else(|e| panic!("{name}: parse_exact rejects a real certificate: {e}"));
        verdict(&der, Mode::Der).unwrap_or_else(|e| panic!("{name}: validate rejects: {e}"));
        verdict(&der, Mode::Ber).unwrap_or_else(|e| panic!("{name}: validate (BER) rejects: {e}"));
    }
}

// Validity ::= SEQUENCE { notBefore Time, notAfter Time }, Time ::= CHOICE { utcTime UTCTime, generalTime
// GeneralizedTime } (RFC 5280 section 4.1.2.5): choose the schema by the tag, then validate the contents.
#[test]
fn validity_times_are_utc_or_generalized() {
    for (name, der) in certs() {
        let root = Tlv::parse_exact(&der, Mode::Der, &Limits::DEFAULT).unwrap();
        let tbs = root.children().next().unwrap().unwrap();
        let mut kids: Vec<Tlv<'_>> = tbs.children().map(Result::unwrap).collect();
        if kids[0].tag().class == Class::ContextSpecific && kids[0].tag().number == 0 {
            kids.remove(0);
        }
        // serialNumber, signature, issuer, validity (RFC 5280 section 4.1).
        let validity = kids[3];
        let times: Vec<Tlv<'_>> = validity.children().map(Result::unwrap).collect();
        assert_eq!(times.len(), 2, "{name}");
        for t in times {
            let prim = match t.tag().number {
                23 => Prim::Utc,
                24 => Prim::Generalized,
                n => panic!("{name}: validity time has tag {n}"),
            };
            let s = Schema::Prim(prim);
            typed::check_identifier(&t.tag(), &s, Mode::Der)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            typed::check_content(&t, &s, Context::new(Mode::Der))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}

/// The root, the version value and the `TBSCertificate` children after the `[0]` version, which start at
/// `serialNumber` (RFC 5280 section 4.1, `version [0] EXPLICIT Version DEFAULT v1`).
fn tbs_fields(der: &[u8]) -> (Tlv<'_>, i128, Vec<Tlv<'_>>) {
    let root = Tlv::parse_exact(der, Mode::Der, &Limits::DEFAULT).unwrap();
    let tbs = root.children().next().unwrap().unwrap();
    let mut kids: Vec<Tlv<'_>> = tbs.children().map(Result::unwrap).collect();
    let mut version = 0;
    if kids[0].tag().class == Class::ContextSpecific && kids[0].tag().number == 0 {
        let v = kids.remove(0).children().next().unwrap().unwrap();
        version = i128::from(*v.content().last().unwrap());
    }
    (root, version, kids)
}

// What the typed layer cannot say about RFC 5280 section 4.1, checked here by hand (the schema above has no ANY,
// no CHOICE and no SIZE):
//  * AlgorithmIdentifier ::= SEQUENCE { algorithm OBJECT IDENTIFIER, parameters ANY ... OPTIONAL } (4.1.1.2):
//    the first component is an OID, there are at most two, and signatureAlgorithm MUST equal tbsCertificate's
//    signature field.
//  * AttributeTypeAndValue ::= SEQUENCE { type OBJECT IDENTIFIER, value ANY }, RelativeDistinguishedName ::= SET
//    SIZE (1..MAX), and the issuer MUST be a non-empty DN (4.1.2.4).
//  * Extensions ::= SEQUENCE SIZE (1..MAX) OF Extension (4.1) and, per the ASN.1 comments, extensions only in a
//    v3 certificate, unique identifiers only in v2 or v3 (4.1.2.8, 4.1.2.9).
#[test]
fn what_the_schema_cannot_say_holds_in_every_certificate() {
    for (name, der) in certs() {
        let (root, version, kids) = tbs_fields(&der);
        let outer: Vec<Tlv<'_>> = root.children().map(Result::unwrap).collect();
        // kids: serialNumber, signature, issuer, validity, subject, subjectPublicKeyInfo, then the optional tail.
        let spki: Vec<Tlv<'_>> = kids[5].children().map(Result::unwrap).collect();
        for (what, alg) in [
            ("tbsCertificate.signature", kids[1]),
            ("signatureAlgorithm", outer[1]),
            ("subjectPublicKeyInfo.algorithm", spki[0]),
        ] {
            let parts: Vec<Tlv<'_>> = alg.children().map(Result::unwrap).collect();
            assert!(
                (1..=2).contains(&parts.len()) && parts[0].tag().number == 6,
                "{name}: {what} is not SEQUENCE {{ OID, parameters OPTIONAL }}"
            );
        }
        assert_eq!(
            kids[1].encoding(),
            outer[1].encoding(),
            "{name}: signatureAlgorithm differs from tbsCertificate.signature (RFC 5280 section 4.1.1.2)"
        );
        for (what, dn) in [("issuer", kids[2]), ("subject", kids[4])] {
            let rdns: Vec<Tlv<'_>> = dn.children().map(Result::unwrap).collect();
            if what == "issuer" {
                assert!(
                    !rdns.is_empty(),
                    "{name}: issuer is empty (section 4.1.2.4)"
                );
            }
            for rdn in rdns {
                let atvs: Vec<Tlv<'_>> = rdn.children().map(Result::unwrap).collect();
                assert!(!atvs.is_empty(), "{name}: {what} has an empty RDN");
                for atv in atvs {
                    let parts: Vec<Tlv<'_>> = atv.children().map(Result::unwrap).collect();
                    assert!(
                        parts.len() == 2 && parts[0].tag().number == 6,
                        "{name}: {what} AttributeTypeAndValue is not {{ OID, value }}"
                    );
                }
            }
        }
        for t in &kids[6..] {
            match t.tag().number {
                1 | 2 => assert!(
                    version >= 1,
                    "{name}: unique identifier in a v1 certificate"
                ),
                3 => {
                    assert_eq!(
                        version, 2,
                        "{name}: extensions in a certificate that is not v3"
                    );
                    let list = t.children().next().unwrap().unwrap();
                    assert!(list.children().count() >= 1, "{name}: empty Extensions");
                }
                n => panic!("{name}: unexpected TBSCertificate field [{n}]"),
            }
        }
    }
}

// RFC 5280 section 4.1.2.5.1 and 4.1.2.5.2: UTCTime is YYMMDDHHMMSSZ, GeneralizedTime is YYYYMMDDHHMMSSZ without
// fractional seconds, and dates through 2049 MUST be UTCTime, 2050 and later MUST be GeneralizedTime. A UTCTime
// reads as 19YY from 50 and 20YY below it, so it cannot express 2050 on its own; the check that remains is that
// a GeneralizedTime is not used for a year the profile reserves for UTCTime.
#[test]
fn validity_times_follow_the_profile_encoding_rules() {
    let mut early: Vec<String> = Vec::new();
    for (name, der) in certs() {
        let (_, _, kids) = tbs_fields(&der);
        for t in kids[3].children().map(Result::unwrap) {
            let text = std::str::from_utf8(t.content()).expect("ASCII time");
            match t.tag().number {
                23 => {
                    assert!(
                        text.len() == 13
                            && text.ends_with('Z')
                            && text[..12].bytes().all(|b| b.is_ascii_digit()),
                        "{name}: UTCTime {text} is not YYMMDDHHMMSSZ"
                    );
                }
                24 => {
                    assert!(
                        text.len() == 15
                            && text.ends_with('Z')
                            && text[..14].bytes().all(|b| b.is_ascii_digit()),
                        "{name}: GeneralizedTime {text} is not YYYYMMDDHHMMSSZ"
                    );
                    let year: u32 = text[..4].parse().unwrap();
                    if year < 2050 {
                        early.push(format!("{name}: {text}"));
                    }
                }
                n => panic!("{name}: validity time has tag {n}"),
            }
        }
    }
    // Certificates that break the MUST on the CA's side (the profile binds the CA, not the reader): a
    // GeneralizedTime for a date before 2050. Listed so that a new one fails the test and a vanished one does too.
    // 23.der is Certum Trusted Network CA 2 (notBefore 2011, notAfter 2046, both GeneralizedTime).
    assert_eq!(
        early,
        ["23.der: 20111006083956Z", "23.der: 20461006083956Z"],
        "certificates using GeneralizedTime before 2050"
    );
}

#[test]
fn berkit_tree_equals_the_reference_dump() {
    let mut total = 0usize;
    let mut maxdepth = 0usize;
    for (name, der) in certs() {
        let root = Tlv::parse_exact(&der, Mode::Der, &Limits::DEFAULT).unwrap();
        let mut ns = Vec::new();
        nodes(root, 0, &mut ns);
        let ours: Vec<String> = ns.iter().map(|(d, t)| dump_line(*d, t)).collect();
        let stem = name.trim_end_matches(".der");
        let theirs =
            fs::read_to_string(data_dir().join(format!("{stem}.tree"))).expect("tree file");
        let theirs: Vec<&str> = theirs.lines().collect();
        assert_eq!(ours.len(), theirs.len(), "{name}: node count");
        for (i, (a, b)) in ours.iter().zip(&theirs).enumerate() {
            assert_eq!(a, b, "{name}: node {i}");
        }
        total += ours.len();
        maxdepth = maxdepth.max(ns.iter().map(|(d, _)| *d).max().unwrap());
        // Every offset and extent berkit reports is inside the file and the header plus contents is the encoding.
        for (_, t) in &ns {
            assert_eq!(
                t.header().len() + t.content().len(),
                t.encoding().len(),
                "{name}"
            );
            assert_eq!(
                &der[t.offset()..t.offset() + t.encoding().len()],
                t.encoding(),
                "{name}"
            );
        }
    }
    eprintln!("compared {total} nodes, deepest level {maxdepth}");
    assert!(total > 2000);
}

// --- mutations ------------------------------------------------------------------------------------------------

fn enc_len(n: usize, pad: usize) -> Vec<u8> {
    if n < 128 && pad == 0 {
        return vec![u8::try_from(n).unwrap()];
    }
    let mut body: Vec<u8> = n
        .to_be_bytes()
        .into_iter()
        .skip_while(|b| *b == 0)
        .collect();
    let mut padded = vec![0u8; pad];
    padded.append(&mut body);
    let mut out = vec![0x80 | u8::try_from(padded.len()).unwrap()];
    out.extend(padded);
    out
}

/// Replace the node at `at` (an offset into the original input) by `ident lenoct content`, re-encoding every
/// ancestor in DER around it.
fn rebuild(t: &Tlv<'_>, at: usize, with: &(Vec<u8>, Vec<u8>, Vec<u8>)) -> Vec<u8> {
    let ident_len = t.tag().encoded_len();
    let content = if t.is_constructed() {
        t.children()
            .flat_map(|c| rebuild(&c.unwrap(), at, with))
            .collect()
    } else {
        t.content().to_vec()
    };
    if t.offset() == at {
        let mut out = with.0.clone();
        out.extend(&with.1);
        out.extend(&with.2);
        return out;
    }
    let mut out = t.header()[..ident_len].to_vec();
    out.extend(enc_len(content.len(), 0));
    out.extend(content);
    out
}

fn replace(root: &Tlv<'_>, target: &Tlv<'_>, lenoct: Vec<u8>, content: Vec<u8>) -> Vec<u8> {
    let ident = target.header()[..target.tag().encoded_len()].to_vec();
    rebuild(root, target.offset(), &(ident, lenoct, content))
}

/// The mutations, by the names `refdump.py` uses.
fn mutate(der: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let root = Tlv::parse_exact(der, Mode::Der, &Limits::DEFAULT).unwrap();
    let mut ns = Vec::new();
    nodes(root, 0, &mut ns);
    let mut out = Vec::new();
    let ident = root.header()[..root.tag().encoded_len()].to_vec();
    let Length::Definite(rl) = root.length() else {
        unreachable!()
    };
    let rl = usize::try_from(rl).unwrap();
    let mut m = ident.clone();
    m.extend(enc_len(rl, 1));
    m.extend(root.content());
    out.push(("len-pad-outer", m));
    let tbs = root.children().next().unwrap().unwrap();
    out.push((
        "len-pad-tbs",
        replace(
            &root,
            &tbs,
            enc_len(tbs.content().len(), 1),
            tbs.content().to_vec(),
        ),
    ));
    let small = ns
        .iter()
        .find(|(d, t)| *d > 0 && t.content().len() < 128)
        .unwrap()
        .1;
    out.push((
        "len-81-short",
        replace(
            &root,
            &small,
            vec![0x81, u8::try_from(small.content().len()).unwrap()],
            small.content().to_vec(),
        ),
    ));
    let mut m = ident;
    m.push(0x80);
    m.extend(root.content());
    m.extend([0, 0]);
    out.push(("indef-outer", m));
    let mut m = der.to_vec();
    m.push(0);
    out.push(("trailing", m));
    out.push(("truncated", der[..der.len() - 1].to_vec()));

    let t: Vec<Tlv<'_>> = tbs.children().map(Result::unwrap).collect();
    let has_ver = t[0].tag().class == Class::ContextSpecific && t[0].tag().number == 0;
    let ser = if has_ver { t[1] } else { t[0] };
    if ser.content()[0] < 0x80 {
        let mut c = vec![0];
        c.extend(ser.content());
        out.push(("serial-pad00", replace(&root, &ser, enc_len(c.len(), 0), c)));
    }
    if has_ver {
        let ver = t[0].children().next().unwrap().unwrap();
        out.push(("version-default", replace(&root, &ver, vec![1], vec![0])));
    }
    let last = t[t.len() - 1];
    if last.tag().class == Class::ContextSpecific && last.tag().number == 3 {
        let exts = last.children().next().unwrap().unwrap();
        let crit = exts.children().map(Result::unwrap).find_map(|x| {
            let k: Vec<Tlv<'_>> = x.children().map(Result::unwrap).collect();
            (k.len() == 3 && k[1].tag().class == Class::Universal && k[1].tag().number == 1)
                .then_some(k[1])
        });
        if let Some(crit) = crit {
            out.push(("bool-true-01", replace(&root, &crit, vec![1], vec![1])));
            out.push((
                "critical-default-false",
                replace(&root, &crit, vec![1], vec![0]),
            ));
        }
    }
    let issuer = t[if has_ver { 3 } else { 2 }];
    let rdn = issuer.children().next().unwrap().unwrap();
    let atv = rdn.children().next().unwrap().unwrap().encoding().to_vec();
    let mut lower = atv.clone();
    *lower.last_mut().unwrap() -= 1;
    let desc: Vec<u8> = [atv.clone(), lower.clone()].concat();
    let asc: Vec<u8> = [lower, atv].concat();
    out.push((
        "setof-descending",
        replace(&root, &rdn, enc_len(desc.len(), 0), desc),
    ));
    out.push((
        "setof-ascending",
        replace(&root, &rdn, enc_len(asc.len(), 0), asc),
    ));
    out
}

/// (mutation, reference prediction, berkit's class) where berkit disagrees with the reference's reading of X.690,
/// and why. Each entry fails the test once berkit stops disagreeing.
const KNOWN: &[(&str, &str, &str, &str)] = &[];

#[test]
fn mutations_get_the_class_the_reference_predicts() {
    let expected =
        fs::read_to_string(data_dir().join("mutations.expected")).expect("mutations.expected");
    let mut want = std::collections::BTreeMap::new();
    for l in expected.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        want.insert(
            (f[0].to_string(), f[1].to_string()),
            (f[2].to_string(), f[3].to_string()),
        );
    }
    let mut seen = 0usize;
    let mut disagreements = BTreeSet::new();
    let mut ber_accepts_padding = 0usize;
    for (cert, der) in certs() {
        for (name, bytes) in mutate(&der) {
            let (class, sha) = want
                .get(&(cert.clone(), name.to_string()))
                .unwrap_or_else(|| panic!("{cert} {name}: the reference built no such mutation"));
            assert_eq!(
                &sha256_hex(&bytes),
                sha,
                "{cert} {name}: the mutated bytes differ from the reference's"
            );
            seen += 1;
            let got = match verdict(&bytes, Mode::Der) {
                Ok(()) => "ok".to_string(),
                Err(c) => c,
            };
            if &got != class {
                disagreements.insert((name.to_string(), class.clone(), got.clone()));
                eprintln!("DISAGREE {cert} {name}: reference {class}, berkit {got}");
            }
            // BER must accept what only DER forbids in the framing: padded lengths and the indefinite form.
            if matches!(
                name,
                "len-pad-outer" | "len-pad-tbs" | "len-81-short" | "indef-outer"
            ) {
                verdict(&bytes, Mode::Ber)
                    .unwrap_or_else(|e| panic!("{cert} {name}: BER rejects: {e}"));
                ber_accepts_padding += 1;
            }
        }
    }
    assert_eq!(
        seen,
        want.len(),
        "mutations built versus mutations the reference built"
    );
    let known: BTreeSet<(String, String, String)> = KNOWN
        .iter()
        .map(|(n, w, g, _)| ((*n).to_string(), (*w).to_string(), (*g).to_string()))
        .collect();
    eprintln!("{seen} mutations compared, {ber_accepts_padding} BER acceptances");
    assert_eq!(
        disagreements, known,
        "disagreements with the reference versus the KNOWN list"
    );
}
