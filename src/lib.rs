//! The C API of louis-rs (louis-rs's ADR-0023). `include/louis.h` is generated from
//! this file with `cbindgen --config cbindgen.toml --output include/louis.h`.
//!
//! Every function checks its pointers. Those that can fail catch panics and report
//! failure as a `LouisStatus`; the message is in `louis_last_error`.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::LazyLock;

use louis::{Direction, TranslationMode, TranslationModes, TranslationOptions, Translator};

/// What a call came to.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)] // the C names
pub enum LouisStatus {
    /// It worked.
    LOUIS_OK = 0,
    /// A pointer was NULL, a string was not UTF-8, or a direction or mode was unknown.
    LOUIS_INVALID_ARGUMENT = 1,
    /// A table could not be found, read or parsed.
    LOUIS_TABLE_ERROR = 2,
    /// The tables were read, but their rules could not be compiled.
    LOUIS_COMPILE_ERROR = 3,
    /// A bug in louis-rs. Please report it, with the message.
    LOUIS_INTERNAL_ERROR = 4,
}
use LouisStatus::*;

/// Translate text to braille.
pub const LOUIS_FORWARD: u32 = 0;
/// Translate braille to text.
pub const LOUIS_BACKWARD: u32 = 1;

/// Translate without contractions (liblouis's `noContractions`). Forward only.
pub const LOUIS_NO_CONTRACTIONS: u32 = 1 << 0;
/// The word at the cursor in computer braille (`compbrlAtCursor`). Forward only.
pub const LOUIS_COMPBRL_AT_CURSOR: u32 = 1 << 1;
/// Only the part of the word before the cursor (`compbrlLeftCursor`). Forward only.
pub const LOUIS_COMPBRL_LEFT_CURSOR: u32 = 1 << 2;
/// Drop characters the tables don't define (`noUndefined`).
pub const LOUIS_NO_UNDEFINED: u32 = 1 << 3;
/// The input may end in the middle of a word (`partialTrans`). Backward only.
pub const LOUIS_PARTIAL_TRANS: u32 = 1 << 4;

const MODES: [(u32, TranslationMode); 5] = [
    (LOUIS_NO_CONTRACTIONS, TranslationMode::NoContractions),
    (LOUIS_COMPBRL_AT_CURSOR, TranslationMode::CompbrlAtCursor),
    (
        LOUIS_COMPBRL_LEFT_CURSOR,
        TranslationMode::CompbrlLeftCursor,
    ),
    (LOUIS_NO_UNDEFINED, TranslationMode::NoUndefined),
    (LOUIS_PARTIAL_TRANS, TranslationMode::PartialTrans),
];

/// Compiled tables for one direction. Immutable: one translator can be used from any
/// number of threads at once.
pub struct LouisTranslator(Translator);

/// The result of `louis_translate`. It owns its output; free it with
/// `louis_result_free`.
pub struct LouisResult {
    /// The output, with a NUL appended for C
    output: String,
    input_positions: Vec<usize>,
    output_positions: Vec<usize>,
    cursor: Option<usize>,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

struct Failure(LouisStatus, String);

fn invalid(message: &str) -> Failure {
    Failure(LOUIS_INVALID_ARGUMENT, message.to_string())
}

/// Run `body`, catch a panic, and record the message of a failure for
/// `louis_last_error`.
fn guard(body: impl FnOnce() -> Result<(), Failure>) -> LouisStatus {
    let (status, message) = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => (LOUIS_OK, None),
        Ok(Err(Failure(status, message))) => (status, Some(message)),
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            (LOUIS_INTERNAL_ERROR, Some(message))
        }
    };
    // a message with a NUL in it is cut there
    let message = message.map(|m| {
        CString::new(m).unwrap_or_else(|e| {
            let end = e.nul_position();
            CString::new(&e.into_vec()[..end]).unwrap()
        })
    });
    LAST_ERROR.with(|last| *last.borrow_mut() = message);
    status
}

/// `n` NUL-terminated UTF-8 strings at `strings`
///
/// # Safety
///
/// `strings` points to `n` valid C strings.
unsafe fn strings<'a>(strings: *const *const c_char, n: usize) -> Result<Vec<&'a str>, Failure> {
    if strings.is_null() {
        return Err(invalid("NULL string array"));
    }
    // SAFETY: the caller promises n readable pointers
    let pointers = unsafe { std::slice::from_raw_parts(strings, n) };
    pointers
        .iter()
        .map(|&p| {
            if p.is_null() {
                return Err(invalid("NULL string"));
            }
            // SAFETY: the caller promises a NUL-terminated string
            unsafe { CStr::from_ptr(p) }
                .to_str()
                .map_err(|_| invalid("string is not UTF-8"))
        })
        .collect()
}

/// Compile `tables` for translating in `direction` (`LOUIS_FORWARD` or `LOUIS_BACKWARD`),
/// and store the translator in `*out`.
///
/// Tables are looked up in the `n_paths` directories of `search_path`, or, if
/// `search_path` is NULL, in those of the `LOUIS_TABLE_PATH` environment variable, or in
/// the current directory if it is not set. All strings are NUL-terminated UTF-8.
///
/// Returns `LOUIS_TABLE_ERROR` or `LOUIS_COMPILE_ERROR` if the tables are broken; this
/// takes the place of liblouis's `lou_checkTable`. On failure, `*out` is NULL.
///
/// # Safety
///
/// `tables` points to `n_tables` C strings, `search_path` is NULL or points to `n_paths`
/// C strings, and `out` is a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_translator_new(
    tables: *const *const c_char,
    n_tables: usize,
    search_path: *const *const c_char,
    n_paths: usize,
    direction: u32,
    out: *mut *mut LouisTranslator,
) -> LouisStatus {
    guard(|| {
        if out.is_null() {
            return Err(invalid("out is NULL"));
        }
        // SAFETY: checked above
        unsafe { *out = ptr::null_mut() };
        let direction = match direction {
            LOUIS_FORWARD => Direction::Forward,
            LOUIS_BACKWARD => Direction::Backward,
            _ => return Err(invalid("unknown direction")),
        };
        // SAFETY: the caller's promises
        let tables = unsafe { strings(tables, n_tables) }?;
        let translator = if search_path.is_null() {
            Translator::new(&tables, direction)
        } else {
            // SAFETY: the caller's promises
            let paths = unsafe { strings(search_path, n_paths) }?;
            Translator::with_search_path(&tables, direction, paths)
        }
        .map_err(|e| {
            let status = match e {
                louis::Error::Table(_) => LOUIS_TABLE_ERROR,
                _ => LOUIS_COMPILE_ERROR,
            };
            Failure(status, e.to_string())
        })?;
        // SAFETY: checked above
        unsafe { *out = Box::into_raw(Box::new(LouisTranslator(translator))) };
        Ok(())
    })
}

/// Free a translator. NULL is ignored.
///
/// # Safety
///
/// `translator` comes from `louis_translator_new` and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_translator_free(translator: *mut LouisTranslator) {
    if !translator.is_null() {
        // SAFETY: the caller's promise
        drop(unsafe { Box::from_raw(translator) });
    }
}

/// Translate the `len` bytes of UTF-8 at `input` with the `LOUIS_*` mode bits in `modes`,
/// and store the result in `*out`. `input` may be NULL if `len` is 0. Modes for the
/// other direction are ignored.
///
/// Braille is Unicode braille: the dot pattern of a cell `c` is `c - 0x2800`, which is
/// what liblouis's `dotsIO` gives. If `cursor` is not NULL, it is a character position
/// in the input, and `louis_result_cursor` gives its position in the output. A cursor
/// past the end of the input is taken as the end.
///
/// Fails only on invalid arguments (NULL pointers, input that is not UTF-8, unknown
/// modes); translation itself always succeeds. On failure, `*out` is NULL.
///
/// # Safety
///
/// `translator` comes from `louis_translator_new`, `input` points to `len` readable
/// bytes (or is NULL if `len` is 0), `cursor` is NULL or valid, and `out` is a valid
/// pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_translate(
    translator: *const LouisTranslator,
    input: *const c_char,
    len: usize,
    modes: u32,
    cursor: *const usize,
    out: *mut *mut LouisResult,
) -> LouisStatus {
    guard(|| {
        if out.is_null() {
            return Err(invalid("out is NULL"));
        }
        // SAFETY: checked above
        unsafe { *out = ptr::null_mut() };
        // SAFETY: the caller's promise
        let Some(LouisTranslator(translator)) = (unsafe { translator.as_ref() }) else {
            return Err(invalid("translator is NULL"));
        };
        let input: &[u8] = if len == 0 {
            &[]
        } else if input.is_null() {
            return Err(invalid("input is NULL"));
        } else {
            // SAFETY: the caller's promise
            unsafe { std::slice::from_raw_parts(input.cast::<u8>(), len) }
        };
        let input = std::str::from_utf8(input).map_err(|_| invalid("input is not UTF-8"))?;
        let all = MODES.iter().fold(0, |all, (bit, _)| all | bit);
        if modes & !all != 0 {
            return Err(invalid("unknown mode"));
        }
        let modes: TranslationModes = MODES
            .iter()
            .filter(|(bit, _)| modes & bit != 0)
            .map(|&(_, mode)| mode)
            .collect();
        let mut options = TranslationOptions::default().with_modes(modes);
        if !cursor.is_null() {
            // SAFETY: the caller's promise
            options = options.with_cursor_position(unsafe { *cursor });
        }
        let result = translator.translate_with_options(input, &options);
        let mut output = result.output().to_string();
        output.push('\0');
        let result = LouisResult {
            output,
            input_positions: result.input_positions().to_vec(),
            output_positions: result.output_positions().to_vec(),
            cursor: result.cursor_position(),
        };
        // SAFETY: checked above
        unsafe { *out = Box::into_raw(Box::new(result)) };
        Ok(())
    })
}

/// Store `value` in `*to` unless `to` is NULL.
///
/// # Safety
///
/// `to` is NULL or valid.
unsafe fn store(to: *mut usize, value: usize) {
    if !to.is_null() {
        // SAFETY: the caller's promise
        unsafe { *to = value };
    }
}

/// The output as NUL-terminated UTF-8, its length in bytes without the NUL in `*len`
/// unless `len` is NULL. Valid until the result is freed. If `result` is NULL, the
/// output is "".
///
/// # Safety
///
/// `result` is NULL or comes from `louis_translate`, and `len` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_result_output(
    result: *const LouisResult,
    len: *mut usize,
) -> *const c_char {
    // SAFETY: the caller's promise
    let output = unsafe { result.as_ref() }.map_or("\0", |r| &r.output);
    // SAFETY: the caller's promise
    unsafe { store(len, output.len() - 1) };
    output.as_ptr().cast()
}

/// The positions in `pick(*result)`, their number in `*len`; none if `result` is NULL.
///
/// # Safety
///
/// `result` is NULL or comes from `louis_translate`, and `len` is NULL or valid.
unsafe fn positions(
    result: *const LouisResult,
    len: *mut usize,
    pick: fn(&LouisResult) -> &[usize],
) -> *const usize {
    // SAFETY: the caller's promise
    let positions = unsafe { result.as_ref() }.map_or(&[][..], pick);
    // SAFETY: the caller's promise
    unsafe { store(len, positions.len()) };
    positions.as_ptr()
}

/// For each output character, the position of the input character it was translated
/// from; their number in `*len` unless `len` is NULL. Positions count characters, not
/// bytes. Valid until the result is freed. If `result` is NULL, there are none.
///
/// # Safety
///
/// `result` is NULL or comes from `louis_translate`, and `len` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_result_input_positions(
    result: *const LouisResult,
    len: *mut usize,
) -> *const usize {
    // SAFETY: the caller's promises
    unsafe { positions(result, len, |r| &r.input_positions) }
}

/// For each input character, the position of its translation in the output; their
/// number in `*len` unless `len` is NULL. Positions count characters, not bytes. Valid
/// until the result is freed. If `result` is NULL, there are none.
///
/// # Safety
///
/// `result` is NULL or comes from `louis_translate`, and `len` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_result_output_positions(
    result: *const LouisResult,
    len: *mut usize,
) -> *const usize {
    // SAFETY: the caller's promises
    unsafe { positions(result, len, |r| &r.output_positions) }
}

/// Whether a cursor was given; if so, its position in the output in `*cursor` unless
/// `cursor` is NULL. False if `result` is NULL.
///
/// # Safety
///
/// `result` is NULL or comes from `louis_translate`, and `cursor` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_result_cursor(
    result: *const LouisResult,
    cursor: *mut usize,
) -> bool {
    // SAFETY: the caller's promise
    match unsafe { result.as_ref() }.and_then(|r| r.cursor) {
        Some(position) => {
            // SAFETY: the caller's promise
            unsafe { store(cursor, position) };
            true
        }
        None => false,
    }
}

/// Free a result. NULL is ignored.
///
/// # Safety
///
/// `result` comes from `louis_translate` and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn louis_result_free(result: *mut LouisResult) {
    if !result.is_null() {
        // SAFETY: the caller's promise
        drop(unsafe { Box::from_raw(result) });
    }
}

/// Why the last call on this thread failed, or NULL if it did not. Valid until the
/// next `louis_*` call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn louis_last_error() -> *const c_char {
    LAST_ERROR.with(|last| last.borrow().as_ref().map_or(ptr::null(), |m| m.as_ptr()))
}

/// The version of louis-rs, such as "0.4.0". This is not the version of the C API,
/// which is in the soname and in `pkg-config --modversion louis-rs`.
#[unsafe(no_mangle)]
pub extern "C" fn louis_version() -> *const c_char {
    static VERSION: LazyLock<CString> = LazyLock::new(|| CString::new(louis::VERSION).unwrap());
    VERSION.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A translator for `tests/test.ctb`
    fn translator(direction: u32) -> *mut LouisTranslator {
        let table = c"test.ctb".as_ptr();
        let path = CString::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")).unwrap();
        let mut out = ptr::null_mut();
        let status =
            unsafe { louis_translator_new(&table, 1, &path.as_ptr(), 1, direction, &mut out) };
        assert_eq!(status, LOUIS_OK);
        assert!(louis_last_error().is_null());
        out
    }

    fn translate(t: *const LouisTranslator, input: &str, modes: u32) -> (String, Vec<usize>) {
        let mut result = ptr::null_mut();
        let status = unsafe {
            louis_translate(
                t,
                input.as_ptr().cast(),
                input.len(),
                modes,
                ptr::null(),
                &mut result,
            )
        };
        assert_eq!(status, LOUIS_OK);
        unsafe {
            let mut len = 0;
            let output = CStr::from_ptr(louis_result_output(result, &mut len));
            assert_eq!(output.to_bytes().len(), len);
            let positions = louis_result_input_positions(result, &mut len);
            let positions = std::slice::from_raw_parts(positions, len).to_vec();
            let output = (output.to_str().unwrap().to_string(), positions);
            louis_result_free(result);
            output
        }
    }

    #[test]
    fn translates_both_ways() {
        let forward = translator(LOUIS_FORWARD);
        assert_eq!(
            translate(forward, "ab ba", 0),
            ("⠹⠀⠃⠁".into(), vec![0, 2, 3, 4])
        );
        assert_eq!(translate(forward, "ab", LOUIS_NO_CONTRACTIONS).0, "⠁⠃");
        let backward = translator(LOUIS_BACKWARD);
        assert_eq!(translate(backward, "⠹⠀⠃⠁", 0).0, "ab ba");
        unsafe {
            louis_translator_free(forward);
            louis_translator_free(backward);
        }
    }

    #[test]
    fn maps_the_cursor() {
        let t = translator(LOUIS_FORWARD);
        let mut result = ptr::null_mut();
        let cursor = 4;
        unsafe {
            let input = "ab ba";
            let status = louis_translate(
                t,
                input.as_ptr().cast(),
                input.len(),
                0,
                &cursor,
                &mut result,
            );
            assert_eq!(status, LOUIS_OK);
            let mut out = 0;
            assert!(louis_result_cursor(result, &mut out));
            assert_eq!(out, 3);
            louis_result_free(result);
            louis_translator_free(t);
        }
    }

    #[test]
    fn reports_bad_arguments() {
        let mut out = ptr::null_mut();
        let table = c"no-such-table.ctb".as_ptr();
        let status =
            unsafe { louis_translator_new(&table, 1, [].as_ptr(), 0, LOUIS_FORWARD, &mut out) };
        assert_eq!(status, LOUIS_TABLE_ERROR);
        assert!(out.is_null());
        assert!(!louis_last_error().is_null());

        let status = unsafe { louis_translator_new(&table, 1, ptr::null(), 0, 7, &mut out) };
        assert_eq!(status, LOUIS_INVALID_ARGUMENT);

        let t = translator(LOUIS_FORWARD);
        let mut result = ptr::null_mut();
        let bad = [0xff_u8];
        let status =
            unsafe { louis_translate(t, bad.as_ptr().cast(), 1, 0, ptr::null(), &mut result) };
        assert_eq!(status, LOUIS_INVALID_ARGUMENT);
        assert!(result.is_null());
        let status =
            unsafe { louis_translate(t, bad.as_ptr().cast(), 0, 1 << 9, ptr::null(), &mut result) };
        assert_eq!(status, LOUIS_INVALID_ARGUMENT);
        unsafe { louis_translator_free(t) };
    }

    #[test]
    fn takes_null() {
        let t = translator(LOUIS_FORWARD);
        let mut result = ptr::null_mut();
        unsafe {
            let status = louis_translate(t, ptr::null(), 0, 0, ptr::null(), &mut result);
            assert_eq!(status, LOUIS_OK);
            assert_eq!(CStr::from_ptr(louis_result_output(result, ptr::null_mut())), c"");
            louis_result_free(result);
            louis_translator_free(t);

            let mut len = 1;
            assert_eq!(CStr::from_ptr(louis_result_output(ptr::null(), &mut len)), c"");
            assert_eq!(len, 0);
            len = 1;
            louis_result_input_positions(ptr::null(), &mut len);
            assert_eq!(len, 0);
            len = 1;
            louis_result_output_positions(ptr::null(), &mut len);
            assert_eq!(len, 0);
            assert!(!louis_result_cursor(ptr::null(), &mut len));
        }
    }

    #[test]
    fn version() {
        let version = unsafe { CStr::from_ptr(louis_version()) };
        assert_eq!(version.to_str().unwrap(), louis::VERSION);
    }
}
