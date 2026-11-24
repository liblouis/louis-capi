# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The C API (`louis_*`) of [louis-rs](https://github.com/liblouis/louis-rs), a Rust
re-implementation of liblouis. It is deliberately *not* a drop-in for liblouis's `lou_*`
API (see louis-rs ADR-0023). Scope is what BRLTTY and TalkBack need; features are added
only when a C consumer asks (the README's "What is missing" lists the gaps — keep it in
sync when adding functions).

louis-rs is a crates.io dependency (`0.4.1`). CI runs `cargo update -p louis-rs` first, so a
new compatible louis-rs release can break the build here.

## Commands

```sh
cargo test                                   # unit tests (in src/lib.rs)
cargo test translates_both_ways              # one test
cargo clippy --all-targets -- -D warnings
cargo +nightly miri test                     # CI runs with MIRIFLAGS=-Zmiri-disable-isolation
cbindgen --config cbindgen.toml --output include/louis.h   # regenerate header (cbindgen 0.29.4)
# C smoke test against the built library:
cargo build && cc -Wall -Wextra -Werror -std=c99 -I include tests/smoke.c \
  -L target/debug -llouis_capi -o target/smoke && LD_LIBRARY_PATH=target/debug target/smoke
# after changing the cargo-c metadata in Cargo.toml, and before a release (not in CI):
cargo cinstall --prefix /tmp/louis --libdir /tmp/louis/lib
# expect include/louis-rs/louis.h, lib/liblouis-rs.so, lib/pkgconfig/louis-rs.pc (Cflags -I${includedir})
```

`include/louis.h` is generated from `src/lib.rs` doc comments and signatures and
committed; CI fails if it is stale. Regenerate after any change to the public API or its
docs. Never hand-edit it.

## Architecture

Everything is in `src/lib.rs`:

- Opaque handles: `LouisTranslator` wraps an immutable `louis::Translator` (shareable
  across threads); `LouisResult` owns the output `String` (with a trailing NUL for C),
  both position maps, and the cursor. Accessors return pointers into the result, valid
  until `louis_result_free`.
- Every fallible `extern "C"` fn returns `LouisStatus` and runs its body through
  `guard()`, which catches panics (→ `LOUIS_INTERNAL_ERROR`) and stores the error
  message in the thread-local `LAST_ERROR` read by `louis_last_error()` (cleared on
  success). Inside, return `Err(Failure(status, msg))` / `invalid(msg)` rather than
  panicking.
- Mode flags (`LOUIS_NO_CONTRACTIONS`, …) map to louis-rs `TranslationMode`s via the
  `MODES` table.
- Text in is UTF-8 with explicit lengths; braille out is Unicode braille (U+2800 block).
- Public fns are `#[unsafe(no_mangle)] pub unsafe extern "C"` with a `# Safety` section;
  `unsafe` blocks carry `// SAFETY:` comments. Tests run under Miri, so keep pointer
  handling Miri-clean.

Tests use the tiny `tests/test.ctb` table (both the Rust tests and `tests/smoke.c`).

cargo-c (`cargo cinstall`) installs it as `liblouis-rs.so`, `louis-rs/louis.h`,
`louis-rs.pc`; the soname follows the crate version, which should change only when the
C API does.
