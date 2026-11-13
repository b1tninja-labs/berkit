# Instructions for coding agents

`berkit` is a Rust workspace (edition 2024, MSRV 1.85) for ASN.1 BER and DER, built clause by clause from the
standard: ITU-T X.690 (02/2021) and X.680 (02/2021).

## The standard is the reference

- `docs/design/` holds the notes each module is built from, written from the standard with clause citations:
  `framing.md`, `primitives.md`, `structured.md`, `strings-time.md`, `annex-examples.md`, and `hazards/`.
  Read the one for your module before you write code. Every rule in them carries a clause; a statement marked
  "not found in X.690" is not a rule.
- Read the standard itself when a note is unclear: the `itulib` MCP tools (`itu_read`, `itu_search`) serve
  X.690 and X.680 by clause. Never state a rule from memory.
- `a sibling directory of the workspace` is a Python implementation. It is NOT a reference and nothing may be copied from it:
  `docs/design/hazards/` records what it got wrong and what ideas are worth keeping.
- Where reference material is missing (X.660, ISO 6093, ISO/IEC 10646, ISO 8601), do not guess: mark the rule
  `[unread]`, say what would settle it, and tell the coordinator so the primarysource sessions can add it.

## Conventions

- `#![no_std]`, no allocation, no dependencies. Functions borrow from the input slice, and encoders write into
  a caller-supplied `&mut [u8]` and return the length written.
- A content-level function takes the contents octets of one value and a `Mode`: `decode(content, mode)`. Its
  offsets count from the start of the contents. Errors are `berkit::Error` with a class from the corpus list.
- Every public item has a doc comment citing its clause in the form `X.690 §8.3.2`.
- Where the standard is silent, the choice is a POLICY: say so in the doc comment, and add a ROADMAP row.
  The policies decided so far are in `ROADMAP.md`.
- A test vector built by hand is labelled `constructed` with its construction; one taken from the standard is
  labelled `from X.690 <place>`. Nothing here comes from a real certificate.

## Commands

All four must pass for your package before a change is done.

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo doc --no-deps

## Invariants

- `unsafe` is forbidden by the lint table; do not relax a lint to make something compile.
- Do not add a dependency without saying why in the commit message. This crate takes none.
- Never run `cargo fmt` or `cargo clippy --fix` outside your own worktree.
- Write scratch output outside the repository.
- A test that cannot fail is not a test. Break the code once, CONFIRM THE EDIT LANDED by printing the changed
  line, watch a test fail, and restore from a backup kept outside the repository; verify the restore with `diff`.
- Commit messages carry no AI attribution trailer.
