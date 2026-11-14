# MSRV policy

The minimum supported Rust version is **1.85**, declared as `rust-version` in the manifest and mirrored
in `clippy.toml` (`msrv`). It is the first release that supports edition 2024.

- CI runs the tests on stable and a job on 1.85 so the claim is tested, not asserted.
- Raising the MSRV is a minor-version change at best, announced in the changelog, and never silent.
- `rust-toolchain.toml` pins the channel `stable` for development only; it does not set the MSRV.

## What was measured (2026-11-13)

Measured locally on Windows, with stable 1.99.0 as the development toolchain:

- Rust 1.85.0: `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.
- 32-bit: the target `i686-pc-windows-msvc` passes all tests.
- Bare metal: the library builds for `thumbv7em-none-eabi` and `thumbv6m-none-eabi`.

The bare-metal result is a build only. Nothing was run on a target, so it shows that the library compiles
without `std` or allocation for those architectures, not that it behaves correctly there. The CI jobs
(`msrv` and `no_std` in `.github/workflows/ci.yml`) repeat the 1.85 tests and the two target builds; they
have not run yet.
