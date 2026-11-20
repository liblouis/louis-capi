# louis-capi

The C API of [louis-rs](https://github.com/liblouis/louis-rs), a Rust
re-implementation of the liblouis braille translator. It is a C API of its own
(`louis_*`), not a replacement for liblouis's `lou_*` functions; see
[ADR-0023](https://github.com/liblouis/louis-rs/blob/main/doc/adr/0023-c-api.org)
for why, and for how to move from liblouis.

Not released yet: it covers what BRLTTY and TalkBack need, see below.

```c
#include <louis-rs/louis.h>

const char *tables[] = {"en-us-g2.ctb"};
LouisTranslator *translator;
if (louis_translator_new(tables, 1, NULL, 0, LOUIS_FORWARD, &translator) != LOUIS_OK)
  fprintf(stderr, "%s\n", louis_last_error());

LouisResult *result;
louis_translate(translator, "hello", 5, 0, NULL, &result);
puts(louis_result_output(result, NULL));   /* ⠓⠑⠇⠇⠕ */
louis_result_free(result);
louis_translator_free(translator);
```

- Text is UTF-8, braille is Unicode braille: a cell's dots are `c - 0x2800`.
- A translator is immutable and can be shared between threads.
- Every result owns its output; free it with `louis_result_free`.

## What is missing

The API covers what the C consumers of liblouis need today, BRLTTY and TalkBack:
building a translator, translating either way with modes and a cursor, both position
maps, the last error and the version. The rest comes when a C consumer asks for it;
please open an issue.

From the louis-rs API, not yet here:

- Braille encodings (display tables), e.g. for BRF output instead of Unicode braille
- Emphasis: as spans of an emphasis class (`{class, start, end}`, not a typeform array)
  for the input, and in the result of back-translation
- The emphasis classes of a translator (`lou_getEmphClasses`)
- Finding tables by their metadata (`lou_findTable`, `lou_findTables`)
- A translator from a table held in memory

Not in louis-rs yet either:

- `lou_charToDots`, `lou_dotsToChar` and `lou_hyphenate`
- A log callback (`lou_registerLogCallback`)
- `lou_getTableInfo` and `lou_listTables`

## Building

`cargo build` builds `liblouis_capi.so` and `.a`. With
[cargo-c](https://github.com/lu-zero/cargo-c), `cargo cinstall` installs
`liblouis-rs.so`, `louis-rs/louis.h` and `louis-rs.pc`.

The header `include/louis.h` is generated: after changing `src/lib.rs`, run
`cbindgen --config cbindgen.toml --output include/louis.h`. CI checks that it is up to
date, runs the tests under Miri and compiles `tests/smoke.c` against the library.

## License

LGPL-2.1-or-later, like louis-rs.
