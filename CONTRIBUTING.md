# Contributing to berkit

1. Install Rust with `rustup`; `rust-toolchain.toml` selects the stable channel. Python 3 (standard library only)
   runs the corpus validator and the matrix generator.
2. Make a change with a test that fails without it. Break the code once, confirm the test fails, and restore it.
3. Run the gates; CI runs the same ones:

       cargo fmt --check
       cargo clippy --all-targets -- -D warnings
       cargo test
       cargo doc --no-deps
       python corpus/tools/check.py
       python tools/matrix.py --check

4. Keep commits small, with a message that says why. Open a pull request against `main`.

Standard rules: a rule of ASN.1 BER or DER is cited from ITU-T X.690 or X.680 by clause, read from the standard
and never from memory. Where the standard is silent the choice is a POLICY: say so in the doc comment and add a
`ROADMAP.md` row. After changing a ROADMAP row run `python tools/matrix.py` to regenerate `docs/matrix.md`.
The corpus is versioned: see `corpus/README.md` before editing a case.

The crate takes no dependencies and is `no_std` without allocation. Compatibility: it builds on Rust 1.85 and
newer. A change that needs a newer compiler is a breaking decision recorded in `docs/msrv.md`, not a drive-by.
