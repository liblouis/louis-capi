# louis-capi

The C API of [louis-rs](https://github.com/liblouis/louis-rs), a Rust
re-implementation of the liblouis braille translator. It is a C API of its own
(`louis_*`), not a replacement for liblouis's `lou_*` functions; see
[ADR-0023](https://github.com/liblouis/louis-rs/blob/main/doc/adr/0023-c-api.org)
for why, and for how to move from liblouis.

It covers what BRLTTY and TalkBack need; see below.

```c
#include <stdio.h>
#include <louis-rs/louis.h>

int main(void) {
  /* looked up in the directories of LOUIS_TABLE_PATH, as search_path is NULL */
  const char *tables[] = {"en-us-g2.ctb"};
  LouisTranslator *translator;
  if (louis_translator_new(tables, 1, NULL, 0, LOUIS_FORWARD, &translator) != LOUIS_OK) {
    fprintf(stderr, "%s\n", louis_last_error());
    return 1;
  }

  LouisResult *result;
  if (louis_translate(translator, "hello", 5, 0, NULL, &result) != LOUIS_OK) {
    fprintf(stderr, "%s\n", louis_last_error());
    louis_translator_free(translator);
    return 1;
  }
  puts(louis_result_output(result, NULL));   /* ⠓⠑⠇⠇⠕ */
  louis_result_free(result);
  louis_translator_free(translator);
  return 0;
}
```

Build it with `cc hello.c $(pkg-config --cflags --libs louis-rs)`.

- Text is UTF-8, braille is Unicode braille: a cell's dots are `c - 0x2800`.
- A translator is immutable and can be shared between threads.
- Every result owns its output; free it with `louis_result_free`.

## Coming from liblouis

| liblouis                                           | louis-rs                                                                                                                   |
|----------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------|
| `lou_translate`, `lou_translateString`             | `louis_translator_new(…, LOUIS_FORWARD, …)` once, then `louis_translate` for each string                                   |
| `lou_backTranslateString`                          | the same with a `LOUIS_BACKWARD` translator                                                                                |
| `lou_getTable`, `lou_checkTable`                   | `louis_translator_new` and its status                                                                                      |
| `lou_setDataPath`, `lou_getDataPath`               | the `search_path` argument; give the tables directory itself, not the data path that liblouis appends `liblouis/tables` to |
| `LOUIS_TABLEPATH` (comma-separated)                | `LOUIS_TABLE_PATH` (separated like `PATH`); without it, tables are looked up in the current directory                      |
| `lou_version`                                      | `louis_version`                                                                                                            |
| `lou_free`                                         | `louis_translator_free`; there is no global cache                                                                          |
| `lou_charSize`, `dotsIO`, `ucBrl`, sizing `outlen` | gone: text is UTF-8, braille is always Unicode braille, and the result owns its output                                     |
| `typeform`                                         | not yet, see below                                                                                                         |

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
[cargo-c](https://github.com/lu-zero/cargo-c),

```sh
cargo cinstall --prefix /usr/local
```

installs `liblouis-rs.so`, `louis-rs/louis.h` and `louis-rs.pc`. The soname and the pkg-config
version are those of the C API, which changes only when the API does; `louis_version()`
gives the version of louis-rs.

The header `include/louis.h` is generated: after changing `src/lib.rs`, run
`cbindgen --config cbindgen.toml --output include/louis.h`. CI checks that it is up to
date, runs the tests under Miri and compiles `tests/smoke.c` against the library.

## License

Copyright (C) 2026 Swiss Library for the Blind, Visually Impaired
and Print Disabled

Licensed under LGPL-2.1-or-later, like louis-rs. See [LICENSE](LICENSE).
