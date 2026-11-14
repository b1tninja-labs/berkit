# berkit

ASN.1 BER and DER in Rust, built clause by clause from ITU-T X.690 (02/2021) and X.680 (02/2021), with a
language-neutral conformance corpus in `corpus/`. The library is `#![no_std]`, allocates nothing and has no
dependencies: functions borrow from the input slice, and encoders write into a caller-supplied `&mut [u8]` and
return the length written. Every public item cites the clause it implements.

berkit is not published to crates.io (`publish = false` in the manifest).

## Status

This is a young library, not a vetted one. What is and is not established:

- **Pinned by tests.** `docs/matrix.md` (generated from the `ROADMAP.md` tables) lists every obligation with
  its clause, its status and the test that pins it: 194 rows, of which 126 are `[ok]` (the named test exists,
  passes, and failed when the code it covers was deliberately broken), 12 are `[unread]` (the test passes but
  the clause could not be read from the standard) and 56 are `[open]`. See `ROADMAP.md` for the open rows.
- **Policy, not standard.** X.690 states the sender's rules and no receiver-side errors, and sets no limits.
  Where berkit rejects, limits or chooses, that is a recorded POLICY decision in `ROADMAP.md`, open to change.
- **Not independent of the author's reading.** The corpus expectations and the code were written from the same
  research notes (`docs/design/`) by one author, and the Python reference decoder in `corpus/tools/` is not
  independent of them. The independent checks are 40 real X.509 CA certificates, whose trees berkit reads
  identically to a separate Python reference walker (`crates/berkit/tests/data/real/`), and the primary-source
  clause readings recorded in the roadmap fragments.
- **Unread references.** Rules that depend on X.660 (OID arcs), ISO 6093 (decimal REAL), ISO/IEC 10646 (UTF-8,
  BMP and UCS-4 rules), ISO 8601 (time) and the ISO register (Teletex, Videotex, Graphic and General string
  alphabets) are marked `[unread]` or `[open]`, not guessed.
- **Not covered yet.** The typed layer validates against a caller-built schema but returns no values and
  has no encoder; berkit is not a certificate parser or an ASN.1 compiler.
- **Targets.** Measured, not asserted: see `docs/msrv.md` (Rust 1.85, a 32-bit target, and a build for two
  bare-metal targets that was not run on a target).

## Use

```rust
use berkit::tlv::{Limits, Tlv};
use berkit::{integer, ErrorClass, Mode};

// 30 03 | 02 01 05: a SEQUENCE of three content octets holding the INTEGER 5.
let der = [0x30, 0x03, 0x02, 0x01, 0x05];
let seq = Tlv::parse_exact(&der, Mode::Der, &Limits::DEFAULT)?;
assert!(seq.is_constructed());

let child = seq.children().next().expect("one child")?;
let n = integer::decode(child.content(), Mode::Der)?;
assert_eq!(n.to_i64(), Some(5));

// Contents 00 05 (the INTEGER 5 with a redundant leading zero octet) are not minimal, X.690 §8.3.2.
let padded = integer::decode(&[0x00, 0x05], Mode::Der);
assert_eq!(padded.unwrap_err().class, ErrorClass::NonMinimalInteger);
# Ok::<(), berkit::Error>(())
```

This is the doc-test in `crates/berkit/src/lib.rs`, run by `cargo test`. Mode (`Mode::Ber` or `Mode::Der`) is
an argument of every decoding function; offsets in an `Error` count from the start of the input given.

## Layout

    crates/berkit/src/     the library: tag, length, tlv (framing); boolean, integer, null, real, bitstring,
                           octetstring, oid, strings, time (content); typed (schema-driven validation)
    crates/berkit/tests/   integration tests, the corpus runner, and data/real (40 real certificates)
    crates/berkit/roadmap/ per-module obligation tables, assembled into ROADMAP.md and docs/matrix.md
    corpus/                the language-neutral conformance corpus (cases, format, schema, validator)
    tools/matrix.py        generates docs/matrix.md from the ROADMAP tables
    docs/design/           the clause-cited notes each module was built from, and hazards/
    docs/                  matrix.md, msrv.md, layout.md
    ROADMAP.md             policy decisions and what is pinned, unread or open

## Gates

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo doc --no-deps
    python corpus/tools/check.py
    python tools/matrix.py --check

Python 3 with the standard library only. CI (`.github/workflows/ci.yml`) runs these, the tests on Rust 1.85, and
a build of the library for `thumbv7em-none-eabi` and `thumbv6m-none-eabi`.

## The corpus

`corpus/` holds 1179 cases (error classes, decoded values, encodings, and the worked examples printed in the
standard) as JSON, versioned and checksummed in `corpus/corpus.json`. An implementation in another language
depends on it by relative path or as a git submodule pinned to a tag, reads `corpus.json`, refuses a different
format or major version, checks each file's sha256, and runs each case in the modes it names. `corpus/format.md`
is the specification of the format; `corpus/README.md` describes the layout and versioning. This crate's runner
is `crates/berkit/tests/corpus.rs`.

## The standard is the reference

Rules are cited from the standard by clause, read from the standard and never written from memory. A statement
in `docs/design/` marked "not found in X.690" is not a rule. Reading clauses uses the primarysource tooling;
where reference material is missing the rule is marked `[unread]` with what would settle it. No code was copied
from another ASN.1 implementation; `docs/design/hazards/` records mistakes found in one and ideas worth keeping.

## Contributing

See `CONTRIBUTING.md`, and `AGENTS.md` for the conventions and invariants.

## Licence

Apache-2.0; see `LICENSE`.
