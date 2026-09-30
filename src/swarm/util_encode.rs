//! Rust port of `packages/core/src/util/encode.ts`.
//!
//! The TypeScript source exports four functions and declares **no data types at
//! all**, no interfaces, no classes, no object literals. Therefore the usual
//! "uppercase field names" trap has no surface in this file: there is not a
//! single serialised field name to get wrong. The only contract that crosses
//! the language boundary is a set of **string formats**, and those are pinned
//! here by exact-literal tests, which is the stronger form of the same check.
//!
//! # Contract summary
//!
//! | TypeScript | Rust | Wire format |
//! |---|---|---|
//! | `base64Encode(s)` | [`base64_encode`] | unpadded base64**url**, `-` and `_` |
//! | `base64Decode(s)` | [`base64_decode`] | lossy UTF-8, never fails on bad bytes |
//! | `hash(s, alg = "SHA-256")` | [`hash`] / [`hash_sha256`] | lowercase hex |
//! | `checksum(s)` | [`checksum`] | FNV-1a 32 over **UTF-16 units**, base36, or `None` |
//! | `sampledChecksum(s, limit = 500_000)` | [`sampled_checksum`] | `"<utf16len>:<c0>:<c1>:<c2>:<c3>:<c4>"`, or `None` |
//!
//! # The four traps this port lives or dies by
//!
//! ## 1. `String::length` is UTF-16 code units, not bytes and not `char`s
//!
//! This is the whole file. `checksum` hashes `content.charCodeAt(i)`, so it
//! consumes **UTF-16 code units**. A "surrogate pair" such as `U+1F389` is two
//! units (`0xD83C`, `0xDF89`), not one, and not four UTF-8 bytes.
//!
//! `sampledChecksum` slices with `content.slice(start, start + size)` and
//! labels the result with `content.length`. Both indices and both lengths are
//! therefore counted in UTF-16 code units. A port that reaches for
//! `content.as_bytes()` or `content.chars()` will produce a *different but
//! plausible* string, and it will never fail to compile.
//!
//! ## 2. A window may cut a surrogate pair in half
//!
//! `content.slice` happily returns a string whose last unit is a lone high
//! surrogate. `checksum` then hashes that raw `0xD83C`. It does **not** round
//! trip through a `String`, and it must not be replaced by `U+FFFD`.
//!
//! A byte-indexed port (`&s[..n]`) does not produce a wrong answer here, it
//! **panics at run time** with no compile error. This module therefore never
//! indexes a `&str` by a computed offset. It materialises a `Vec<u16>` once
//! and slices *that*, which cannot panic and cannot lose a byte.
//!
//! ## 3. Truthiness (`!content`) is not nullity (`?? ""`)
//!
//! `checksum` and `sampledChecksum` both open with `if (!content) return
//! undefined`. That is a **truthiness** test on a JavaScript string, so the
//! empty string is falsy and yields `None` / `undefined`. A `??` test would
//! have let the empty string through. The only `??` in the source is
//! `checksum(content.slice(...)) ?? ""`, which is a **nullity** test on a
//! function result, not on user input, and is reproduced separately by
//! [`sampled_checksum_with_limit`]. The two are never merged here.
//!
//! By construction the sampled window is never empty (`start` is always
//! `<= length - SAMPLE_SIZE` when the content is at least that long, and
//! `0` otherwise, while the content itself is known non-empty), so the `?? ""`
//! branch is dead in the TypeScript too. It is kept anyway, for parity.
//!
//! ## 4. `TextDecoder` is lossy and says nothing
//!
//! `new TextDecoder().decode(bytes)` is constructed without `{ fatal: true }`,
//! so `base64Decode` of arbitrary bytes **cannot** fail: every malformed UTF-8
//! sequence silently becomes `U+FFFD`. [`base64_decode`] reproduces that
//! exactly, because silently substituting a replacement character is the
//! documented behaviour callers already depend on. [`base64_decode_strict`] is
//! offered alongside it for callers that would rather be told.
//!
//! # Deliberate differences
//!
//! - `hash` is synchronous here (Rust has no `Promise`); the bytes it digests
//!   are identical to `new TextEncoder().encode(content)`, i.e. UTF-8.
//! - `crypto.subtle.digest` also accepts `SHA-384` and `SHA-512`. They are not
//!   implemented, and [`hash_named`] returns
//!   [`HashError::UnsupportedAlgorithm`] rather than silently substituting
//!   something else. SHA-1 and SHA-256 are reused from the sibling module
//!   `util_hash`, so the two ports cannot drift apart.
//! - Errors are values (`Result`), not thrown exceptions. `atob` throws on a
//!   bad character or a bad length; the same inputs are `Err` here.

use std::error::Error;
use std::fmt;

use super::util_hash;

/// Number of UTF-16 code units hashed in each window by [`sampled_checksum`].
pub const SAMPLE_SIZE: usize = 4096;

/// Content shorter than or equal to this many UTF-16 code units is hashed whole
/// instead of being sampled. Default of the `limit` parameter.
pub const DEFAULT_SAMPLE_LIMIT: usize = 500_000;

/// The 64 symbols of unpadded base64url, used by [`base64_encode`].
///
/// The TypeScript encodes with the standard alphabet and then rewrites
/// `+` -> `-`, `/` -> `_` and drops every `=`. Emitting this alphabet directly
/// is the same function, without the intermediate string.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

// ---------------------------------------------------------------------------
// base64
// ---------------------------------------------------------------------------

/// Encodes `value` as UTF-8 and returns unpadded base64url.
///
/// Port of `base64Encode`. The output is never empty-padded and never contains
/// `+`, `/` or `=`, which makes it safe to drop into a URL path segment or a
/// file name without further escaping.
pub fn base64_encode(value: &str) -> String {
    encode_bytes(value.as_bytes())
}

/// The byte-level half of [`base64_encode`], for callers holding raw bytes
/// rather than text.
pub fn base64_encode_bytes(bytes: &[u8]) -> String {
    encode_bytes(bytes)
}

fn encode_bytes(bytes: &[u8]) -> String {
    // 4 output symbols per 3 input bytes, rounded up, minus the padding that
    // unpadded base64url drops.
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    out
}

/// Everything that can go wrong while decoding base64url.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base64DecodeError {
    /// A byte outside the base64 alphabet survived normalisation, e.g. `!`,
    /// a stray `=`, or any byte `>= 0x80`. Carries the offending byte.
    InvalidCharacter(u8),
    /// The symbol count leaves 1 over a group of 4, which cannot encode a
    /// whole byte. Carries the symbol count.
    InvalidLength(usize),
    /// The bytes decoded cleanly but are not valid UTF-8. Only ever returned
    /// by [`base64_decode_strict`].
    InvalidUtf8,
}

impl fmt::Display for Base64DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Base64DecodeError::InvalidCharacter(byte) => {
                write!(f, "invalid base64 character 0x{byte:02x}")
            }
            Base64DecodeError::InvalidLength(len) => {
                write!(f, "invalid base64 length {len}")
            }
            Base64DecodeError::InvalidUtf8 => f.write_str("decoded bytes are not valid UTF-8"),
        }
    }
}

impl Error for Base64DecodeError {}

/// Decodes base64url (or padded standard base64) into raw bytes.
///
/// Port of the body of `base64Decode`, stopping before the text step. This is
/// the only place where the lossiness of the operation is visible; see
/// [`base64_decode`] and [`base64_decode_strict`].
///
/// Accepts, exactly like `atob`:
/// - both alphabets, because `-` is folded to `+` and `_` is folded to `/`;
/// - missing padding, and padding that is present and well formed;
/// - ASCII whitespace anywhere (TAB, LF, FF, CR, SPACE), which is stripped.
///
/// Rejects, exactly like `atob`:
/// - any other character, including a `=` in the middle and a `=` in a string
///   whose length is not a multiple of 4;
/// - a length that leaves 1 symbol over a group of 4.
///
/// Bits below the last whole byte are discarded rather than checked, matching
/// `atob`: `"Zg"` and `"Zh"` both decode to `"f"`.
pub fn base64_decode_bytes(value: &str) -> Result<Vec<u8>, Base64DecodeError> {
    let mut body: Vec<u8> = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    for byte in &mut body {
        match *byte {
            b'-' => *byte = b'+',
            b'_' => *byte = b'/',
            _ => {}
        }
    }

    // `atob` strips padding only when the whole string is a multiple of 4, and
    // strips at most two symbols. "====" therefore collapses to "==" and is
    // then rejected, instead of being silently accepted.
    if body.len() % 4 == 0 {
        let mut stripped = 0;
        while stripped < 2 && body.last() == Some(&b'=') {
            body.pop();
            stripped += 1;
        }
    }

    let mut symbols: Vec<u8> = Vec::with_capacity(body.len());
    for &byte in &body {
        match symbol_value(byte) {
            Some(value) => symbols.push(value),
            None => return Err(Base64DecodeError::InvalidCharacter(byte)),
        }
    }

    // 1 symbol over a group of 4 carries no whole byte, so it cannot be valid.
    // Checking here also guarantees every chunk below has at least 2 symbols.
    if symbols.len() % 4 == 1 {
        return Err(Base64DecodeError::InvalidLength(symbols.len()));
    }

    let mut out = Vec::with_capacity(symbols.len() / 4 * 3 + 2);
    for chunk in symbols.chunks(4) {
        let n = (u32::from(chunk[0]) << 18)
            | (u32::from(chunk[1]) << 12)
            | (u32::from(chunk.get(2).copied().unwrap_or(0)) << 6)
            | u32::from(chunk.get(3).copied().unwrap_or(0));
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

fn symbol_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes base64url to text, replacing malformed UTF-8 with `U+FFFD`.
///
/// Port of `base64Decode`. The replacement is **not** an error: the source
/// builds a `TextDecoder` with no `fatal` option, so invalid byte sequences
/// come back as `U+FFFD` and nothing is reported. Use
/// [`base64_decode_strict`] when silent corruption is worse than a failure.
pub fn base64_decode(value: &str) -> Result<String, Base64DecodeError> {
    let bytes = base64_decode_bytes(value)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Decodes base64url to text, refusing malformed UTF-8.
///
/// Same as [`base64_decode`] except that invalid UTF-8 becomes
/// [`Base64DecodeError::InvalidUtf8`] instead of a string full of `U+FFFD`. The
/// TypeScript has no counterpart for this; it exists because a round trip that
/// quietly rewrites bytes is the kind of bug that only shows up three services
/// downstream.
pub fn base64_decode_strict(value: &str) -> Result<String, Base64DecodeError> {
    let bytes = base64_decode_bytes(value)?;
    String::from_utf8(bytes).map_err(|_| Base64DecodeError::InvalidUtf8)
}

// ---------------------------------------------------------------------------
// hash
// ---------------------------------------------------------------------------

/// The digests this port implements.
///
/// `crypto.subtle.digest` also accepts `SHA-384` and `SHA-512`; see
/// [`hash_named`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
}

/// `hash` was asked for a digest this port does not implement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashError {
    /// The name exactly as the caller spelled it.
    pub requested: String,
}

impl fmt::Display for HashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported hash algorithm {:?}; this port implements SHA-1 and SHA-256",
            self.requested
        )
    }
}

impl Error for HashError {}

/// Digests the UTF-8 bytes of `content` and returns lowercase hex.
///
/// Port of `hash`. The output is two lowercase hex digits per byte, so a
/// SHA-256 is always 64 characters. `content` is encoded as UTF-8 first, which
/// is what `new TextEncoder().encode(content)` does.
pub fn hash(content: &str, algorithm: HashAlgorithm) -> String {
    match algorithm {
        // `util_hash::fast` is SHA-1 despite the name; the sibling module
        // documents this. Reusing it here keeps one SHA-1 in the tree.
        HashAlgorithm::Sha1 => util_hash::fast(content.as_bytes()),
        HashAlgorithm::Sha256 => util_hash::sha256(content.as_bytes()),
    }
}

/// [`hash`] with the default algorithm of the source, `SHA-256`.
pub fn hash_sha256(content: &str) -> String {
    hash(content, HashAlgorithm::Sha256)
}

/// Resolves a WebCrypto algorithm name and digests `content`.
///
/// Port of `hash(content, algorithm)`. The name is matched case-insensitively
/// and surrounding whitespace is ignored, as WebCrypto does.
///
/// `SHA-384` and `SHA-512` are accepted by `crypto.subtle.digest` and are **not**
/// implemented here. They return [`HashError`] rather than falling back to
/// SHA-256, because a caller that asked for SHA-512 and silently received
/// SHA-256 has a security problem, not a convenience.
pub fn hash_named(content: &str, algorithm: &str) -> Result<String, HashError> {
    match algorithm.trim().to_ascii_uppercase().as_str() {
        "SHA-1" => Ok(hash(content, HashAlgorithm::Sha1)),
        "SHA-256" => Ok(hash(content, HashAlgorithm::Sha256)),
        _ => Err(HashError {
            requested: algorithm.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// checksum
// ---------------------------------------------------------------------------

/// FNV-1a, 32-bit, over UTF-16 code units, rendered in base 36.
///
/// Port of `checksum`. `None` stands for the source's `undefined`.
///
/// Two details make this the fragile function of the file:
///
/// - `content.charCodeAt(i)` walks **UTF-16 code units**. A non-BMP character
///   is hashed as its two surrogate halves, so `checksum("\u{1F389}")` is not
///   the checksum of any single scalar value.
/// - `hash >>> 0` is a reinterpretation of a signed 32-bit JavaScript integer
///   as unsigned. Working in `u32` reproduces it exactly, because JavaScript
///   bitwise operators and `Math.imul` are already two's complement modulo
///   2^32.
///
/// The empty string is falsy in JavaScript, so it returns `None`. That is a
/// truthiness test, and it is deliberately not the same thing as a null check.
pub fn checksum(content: &str) -> Option<String> {
    if content.is_empty() {
        return None;
    }
    Some(to_base36(fnv1a(content.encode_utf16())))
}

fn fnv1a<I>(units: I) -> u32
where
    I: IntoIterator<Item = u16>,
{
    let mut hash = FNV_OFFSET_BASIS;
    for unit in units {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Renders `value` the way `Number.prototype.toString(36)` does: lowercase
/// `0-9a-z`, most significant first, no padding, and `"0"` for zero.
fn to_base36(value: u32) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    // 36^7 > u32::MAX, so seven digits is always enough.
    let mut reversed: Vec<u8> = Vec::with_capacity(7);
    let mut rest = value;
    loop {
        reversed.push(DIGITS[(rest % 36) as usize]);
        rest /= 36;
        if rest == 0 {
            break;
        }
    }
    reversed.reverse();
    // Every byte came from an ASCII table, so this cannot fail.
    String::from_utf8(reversed).unwrap_or_default()
}

/// Port of `sampledChecksum` with its default `limit`.
pub fn sampled_checksum(content: &str) -> Option<String> {
    sampled_checksum_with_limit(content, DEFAULT_SAMPLE_LIMIT)
}

/// Port of `sampledChecksum(content, limit)`.
///
/// Returns `None` for empty input, matching the `!content` truthiness guard.
/// Content of at most `limit` UTF-16 code units is hashed whole, with no
/// prefix. Longer content is sampled: five windows of [`SAMPLE_SIZE`] code
/// units are hashed and the result is `"<utf16 length>:<c0>:<c1>:<c2>:<c3>:<c4>"`.
///
/// The windows are indexed and measured in **UTF-16 code units**, so a window
/// boundary can land between the two halves of a surrogate pair. Those windows
/// are hashed over their raw code units, lone surrogate included, exactly as
/// `content.slice` followed by `checksum` does. The content is materialised
/// as a `Vec<u16>` so that no slice can fall between the bytes of a character
/// and panic.
pub fn sampled_checksum_with_limit(content: &str, limit: usize) -> Option<String> {
    if content.is_empty() {
        return None;
    }

    let units: Vec<u16> = content.encode_utf16().collect();
    let length = units.len();

    if length <= limit {
        return checksum(content);
    }

    // `Math.floor(content.length * 0.25)` and friends. 0.25, 0.5 and 0.75 are
    // exact in binary floating point, so the floor is a plain integer
    // division. The 3/4 term is split so that it cannot overflow `usize`.
    let points = [
        0usize,
        length / 4,
        length / 2,
        (length / 4) * 3 + (length % 4) * 3 / 4,
        length.saturating_sub(SAMPLE_SIZE),
    ];

    let mut parts: Vec<String> = Vec::with_capacity(points.len());
    for &point in &points {
        // `Math.max(0, Math.min(length - size, point - Math.floor(size / 2)))`,
        // with both subtractions saturating instead of wrapping, because in
        // JavaScript they produce negative numbers that the outer max clamps.
        let start = point
            .saturating_sub(SAMPLE_SIZE / 2)
            .min(length.saturating_sub(SAMPLE_SIZE));
        // `String.prototype.slice` clamps the end index to the string length.
        let end = (start + SAMPLE_SIZE).min(length);
        // `?? ""` is a null check on the result, not on the input. The window
        // cannot actually be empty here, but the fallback is kept for parity.
        parts.push(window_checksum(&units[start..end]).unwrap_or_default());
    }

    Some(format!("{}:{}", length, parts.join(":")))
}

/// `checksum` applied to a window that may contain a lone surrogate.
///
/// Kept separate from [`checksum`] on purpose: going through a `String` would
/// turn that lone surrogate into `U+FFFD` and change the digest.
fn window_checksum(units: &[u16]) -> Option<String> {
    if units.is_empty() {
        return None;
    }
    Some(to_base36(fnv1a(units.iter().copied())))
}

#[cfg(test)]
mod tests {
    use super::{
        base64_decode, base64_decode_bytes, base64_decode_strict, base64_encode,
        base64_encode_bytes, checksum, hash, hash_named, hash_sha256, sampled_checksum,
        sampled_checksum_with_limit, to_base36, Base64DecodeError, HashAlgorithm, HashError,
        DEFAULT_SAMPLE_LIMIT, SAMPLE_SIZE,
    };

    // A string built from every width of character the file has to survive:
    // ASCII, a two-byte Latin-1 supplement code point, a three-byte CJK code
    // point, a four-byte non-BMP code point, and an embedded NUL.
    const MIXED: &str = "a\u{e9}\u{65e5}\u{1F389}\u{0}z";

    // -- base64 encoding ----------------------------------------------------

    #[test]
    fn the_rfc_4648_vectors_encode_without_padding() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg");
        assert_eq!(base64_encode("fo"), "Zm8");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("foob"), "Zm9vYg");
        assert_eq!(base64_encode("fooba"), "Zm9vYmE");
        assert_eq!(base64_encode("foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_output_never_contains_padding_or_standard_alphabet_symbols() {
        // U+00DF is 0xC3 0x9F, which is "w5/" in standard base64: the last
        // symbol is index 63 and becomes '_' here, and the padding vanishes.
        assert_eq!(base64_encode("\u{df}"), "w58");
        assert_eq!(base64_encode_bytes(&[0xff]), "_w");
        assert_eq!(base64_encode_bytes(&[0xef, 0xbf]), "778");
        assert_eq!(base64_encode_bytes(&[0xfb, 0xef, 0xbe]), "----");
        assert_eq!(base64_encode_bytes(&[0xff, 0xff, 0xff]), "____");
        // Every value that the standard alphabet maps to '+' or '/'.
        for octet in [0xfb, 0xfc, 0xfd, 0xfe, 0xff] {
            for length in 1..=3usize {
                let encoded = base64_encode_bytes(&vec![octet; length]);
                assert!(!encoded.contains('+'), "{octet:#04x} x{length}");
                assert!(!encoded.contains('/'), "{octet:#04x} x{length}");
                assert!(!encoded.contains('='), "{octet:#04x} x{length}");
                assert!(!encoded.is_empty());
            }
        }
    }

    #[test]
    fn non_ascii_input_is_encoded_from_its_utf8_bytes() {
        assert_eq!(base64_encode("\u{65e5}\u{672c}\u{8a9e}"), "5pel5pys6Kqe");
        assert_eq!(base64_encode("a\u{0}b"), "YQBi");
    }

    #[test]
    fn every_input_length_round_trips_including_empty_and_multi_byte() {
        let alphabet = ['a', '\u{e9}', '\u{65e5}', '\u{1F389}', '\u{0}', '\u{7f}'];
        let mut content = String::new();
        for index in 0..64usize {
            content.push(alphabet[index % alphabet.len()]);
            let encoded = base64_encode(&content);
            assert!(!encoded.contains('='));
            assert_eq!(base64_decode(&encoded), Ok(content.clone()));
        }
    }

    // -- base64 decoding ----------------------------------------------------

    #[test]
    fn an_empty_string_decodes_to_an_empty_string() {
        assert_eq!(base64_decode(""), Ok(String::new()));
        assert_eq!(base64_decode_bytes(""), Ok(Vec::new()));
    }

    #[test]
    fn missing_padding_is_accepted() {
        assert_eq!(base64_decode("Zg"), Ok("f".to_string()));
        assert_eq!(base64_decode("Zm8"), Ok("fo".to_string()));
        assert_eq!(base64_decode("Zm9"), Ok("fo".to_string()));
        assert_eq!(base64_decode("Zm9v"), Ok("foo".to_string()));
        assert_eq!(base64_decode("Zm9vYg"), Ok("foob".to_string()));
    }

    #[test]
    fn correct_padding_is_accepted_and_pared_standard_base64_is_accepted() {
        assert_eq!(base64_decode("Zg=="), Ok("f".to_string()));
        assert_eq!(base64_decode("Zm8="), Ok("fo".to_string()));
        assert_eq!(base64_decode("Zm9vYg=="), Ok("foob".to_string()));
        // Standard alphabet, from btoa() before the substitution pass.
        assert_eq!(base64_decode("w5//"), Ok("\u{df}\u{fffd}".to_string()));
    }

    #[test]
    fn ascii_whitespace_is_stripped_anywhere() {
        assert_eq!(base64_decode(" Zh\n"), Ok("f".to_string()));
        // `\f` is not a Rust escape sequence: the ASCII form feed is `\u{c}`.
        assert_eq!(base64_decode("Z\r\nm9\tv\u{c}"), Ok("foo".to_string()));
        assert_eq!(base64_decode(" Zm9v "), Ok("foo".to_string()));
    }

    #[test]
    fn trailing_bits_below_the_last_whole_byte_are_discarded() {
        // "Zg" and "Zh" differ only in the four bits `atob` throws away.
        assert_eq!(base64_decode("Zh"), base64_decode("Zg"));
        assert_eq!(base64_decode("Zh"), Ok("f".to_string()));
    }

    #[test]
    fn an_unknown_character_is_rejected() {
        assert_eq!(
            base64_decode("Zm9v!"),
            Err(Base64DecodeError::InvalidCharacter(b'!'))
        );
        assert_eq!(base64_decode("@@@@"), Err(Base64DecodeError::InvalidCharacter(b'@')));
        // A non-ASCII character must be rejected, never truncated to a byte.
        assert_eq!(base64_decode("\u{e9}"), Err(Base64DecodeError::InvalidCharacter(0xc3)));
        assert_eq!(base64_decode("\u{1f389}"), Err(Base64DecodeError::InvalidCharacter(0xf0)));
    }

    #[test]
    fn a_length_leaving_one_symbol_over_four_is_rejected() {
        assert_eq!(base64_decode("A"), Err(Base64DecodeError::InvalidLength(1)));
        assert_eq!(base64_decode("AAAAA"), Err(Base64DecodeError::InvalidLength(5)));
        assert_eq!(base64_decode("Zm9vA"), Err(Base64DecodeError::InvalidLength(5)));
    }

    #[test]
    fn misplaced_padding_is_rejected() {
        // Padding is only stripped when the length is a multiple of 4, so all
        // of these keep a '=' that is not in the alphabet.
        assert_eq!(base64_decode("Zm9v="), Err(Base64DecodeError::InvalidCharacter(b'=')));
        assert_eq!(base64_decode("Zm9v=="), Err(Base64DecodeError::InvalidCharacter(b'=')));
        assert_eq!(base64_decode("Zm9v==="), Err(Base64DecodeError::InvalidCharacter(b'=')));
        // Four '=' collapse to two, which are still not in the alphabet.
        assert_eq!(base64_decode("===="), Err(Base64DecodeError::InvalidCharacter(b'=')));
        // A '=' in the middle, with a length that is a multiple of 4.
        assert_eq!(base64_decode("Zg==Zg=="), Err(Base64DecodeError::InvalidCharacter(b'=')));
    }

    #[test]
    fn invalid_utf8_is_replaced_silently_by_the_lossy_decoder() {
        // 0xFF 0xFF 0xFF is not UTF-8. The source's TextDecoder is not fatal,
        // so this is a success carrying three replacement characters.
        let decoded = base64_decode("____").expect("base64 level is valid");
        assert_eq!(decoded, "\u{fffd}\u{fffd}\u{fffd}");
        assert_eq!(base64_decode_bytes("____"), Ok(vec![0xff, 0xff, 0xff]));
    }

    #[test]
    fn the_strict_decoder_reports_what_the_lossy_one_hides() {
        assert_eq!(base64_decode_strict("____"), Err(Base64DecodeError::InvalidUtf8));
        assert_eq!(base64_decode_strict("w5//"), Err(Base64DecodeError::InvalidUtf8));
        // A well formed payload behaves identically on both decoders.
        assert_eq!(base64_decode_strict("Zm9v"), Ok("foo".to_string()));
        assert_eq!(base64_decode_strict("Zm9v"), base64_decode("Zm9v"));
        // A truncated multi-byte character is exactly the silent case: the
        // first byte survives, the rest becomes U+FFFD.
        assert_eq!(base64_decode("w58"), Ok("\u{df}".to_string()));
        assert_eq!(base64_decode("4A"), Ok("\u{fffd}".to_string()));
        assert_eq!(base64_decode_strict("4A"), Err(Base64DecodeError::InvalidUtf8));
    }

    #[test]
    fn error_messages_name_the_problem() {
        let invalid_character = Base64DecodeError::InvalidCharacter(b'!');
        assert!(invalid_character.to_string().contains("0x21"));
        let invalid_length = Base64DecodeError::InvalidLength(5);
        assert!(invalid_length.to_string().contains('5'));
        assert!(Base64DecodeError::InvalidUtf8
            .to_string()
            .contains("UTF-8"));
    }

    // -- hash ---------------------------------------------------------------

    #[test]
    fn sha_256_matches_the_published_digests() {
        assert_eq!(
            hash_sha256(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash_sha256("\u{65e5}\u{672c}\u{8a9e}"),
            "77710aedc74ecfa33685e33a6c7df5cc83004da1bdcef7fb280f5c2b2e97e0a5"
        );
    }

    #[test]
    fn sha_1_matches_the_published_digest() {
        assert_eq!(
            hash("abc", HashAlgorithm::Sha1),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hash_sha256(""), hash("", HashAlgorithm::Sha256));
    }

    #[test]
    fn digests_are_lowercase_hex_of_the_expected_width() {
        let sha256 = hash_sha256(MIXED);
        assert_eq!(sha256.len(), 64);
        let sha1 = hash(MIXED, HashAlgorithm::Sha1);
        assert_eq!(sha1.len(), 40);
        for digest in [&sha256, &sha1] {
            assert!(digest
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
        }
        assert_ne!(hash_sha256("a"), hash_sha256("A"));
        assert_ne!(hash_sha256("a\u{0}b"), hash_sha256("ab"));
    }

    #[test]
    fn algorithm_names_are_matched_case_insensitively_and_trimmed() {
        for name in ["SHA-256", "sha-256", "  Sha-256\t"] {
            assert_eq!(hash_named("abc", name), Ok(hash_sha256("abc")));
        }
        for name in ["SHA-1", "sha-1"] {
            assert_eq!(hash_named("abc", name), Ok(hash("abc", HashAlgorithm::Sha1)));
        }
    }

    #[test]
    fn an_unimplemented_algorithm_is_an_error_and_not_a_silent_fallback() {
        for name in ["SHA-512", "SHA-384", "MD5", "", "sha256"] {
            let error = hash_named("abc", name).expect_err("must not fall back");
            assert_eq!(error, HashError { requested: name.to_string() });
            assert!(error.to_string().contains("unsupported"));
        }
    }

    // -- checksum -----------------------------------------------------------

    #[test]
    fn checksum_matches_the_typescript_values() {
        assert_eq!(checksum("a").as_deref(), Some("1r9wi7g"));
        assert_eq!(checksum("ab").as_deref(), Some("lekqmi"));
        assert_eq!(checksum("e").as_deref(), Some("1q5y3fk"));
        assert_eq!(checksum("opencode").as_deref(), Some("ql3u3k"));
    }

    #[test]
    fn the_empty_string_is_falsy_and_yields_no_checksum() {
        // This is `if (!content) return undefined`, a truthiness test. A null
        // check would have returned a value here.
        assert_eq!(checksum(""), None);
        assert_eq!(sampled_checksum(""), None);
        assert_eq!(sampled_checksum_with_limit("", 0), None);
    }

    #[test]
    fn a_string_of_zeroes_is_truthy_and_does_produce_a_checksum() {
        // "0" is a non-empty string, so it is truthy in JavaScript. Only the
        // empty string is falsy, and it is the only input that yields None.
        assert!(checksum("0").is_some());
        assert!(checksum("\u{0}").is_some());
    }

    #[test]
    fn base36_rendering_matches_javascript() {
        assert_eq!(to_base36(0), "0");
        assert_eq!(to_base36(1), "1");
        assert_eq!(to_base36(35), "z");
        assert_eq!(to_base36(36), "10");
        assert_eq!(to_base36(u32::MAX), "1z141z3");
        // Every rendered checksum is base36 and at most 7 characters.
        for content in ["a", "opencode", MIXED] {
            let digest = checksum(content).expect("non empty");
            assert!(!digest.is_empty());
            assert!(digest.len() <= 7);
            assert!(digest
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='z').contains(&c)));
        }
    }

    #[test]
    fn the_checksum_hashes_utf16_code_units_not_bytes_and_not_scalars() {
        // U+1F389 is two code units, so it has a different digest from any
        // single scalar value, and from a two-character ASCII string of the
        // same scalar count.
        assert_eq!(checksum("\u{1f389}").as_deref(), Some("swncdy"));
        assert_ne!(checksum("\u{1f389}"), checksum("\u{fffd}"));
        assert_ne!(checksum("\u{1f389}"), checksum("ab"));
        // A two-byte character is one code unit but two bytes. Hashing the
        // bytes would hash U+00C3 U+00A9 instead, giving 8htmm9 rather than
        // the digest of the single code unit 0x00E9, which is tz86ys.
        assert_ne!(checksum("\u{e9}").as_deref(), checksum("\u{c3}\u{a9}").as_deref());
        // Same scalar count, same code unit count, different units.
        assert_ne!(checksum("ab").as_deref(), checksum("\u{e9}\u{1}").as_deref());
    }

    #[test]
    fn the_checksum_is_not_the_digest_of_the_utf8_bytes() {
        // U+00E9 is one code unit (0x00E9) but two UTF-8 bytes (0xC3 0xA9).
        // A byte-based port would hash the two code units U+00C3 and U+00A9
        // instead, and produce 8htmm9.
        assert_eq!(checksum("\u{e9}").as_deref(), Some("tz86ys"));
        assert_eq!(checksum("\u{c3}\u{a9}").as_deref(), Some("8htmm9"));
        assert_ne!(checksum("\u{e9}"), checksum("\u{c3}\u{a9}"));
        // Three CJK characters, one code unit each, one digest.
        assert_eq!(checksum("\u{65e5}\u{672c}\u{8a9e}").as_deref(), Some("nbamha"));
    }

    #[test]
    fn an_embedded_nul_is_hashed_like_any_other_code_unit() {
        assert_ne!(checksum("a\u{0}b"), checksum("ab"));
        assert_ne!(checksum("\u{0}"), checksum(""));
    }

    // -- sampled_checksum ---------------------------------------------------

    #[test]
    fn content_at_or_below_the_limit_is_hashed_whole_with_no_prefix() {
        assert_eq!(sampled_checksum("opencode"), checksum("opencode"));
        // Exactly at the limit: still the short branch.
        let at_limit = "a".repeat(DEFAULT_SAMPLE_LIMIT);
        assert_eq!(sampled_checksum(&at_limit), checksum(&at_limit));
        assert!(!sampled_checksum(&at_limit).unwrap().contains(':'));
    }

    #[test]
    fn one_code_unit_past_the_limit_switches_to_the_sampled_branch() {
        let over = "a".repeat(DEFAULT_SAMPLE_LIMIT + 1);
        let sampled = sampled_checksum(&over).expect("non empty");
        assert!(sampled.starts_with(&format!("{}:", DEFAULT_SAMPLE_LIMIT + 1)));
        assert_eq!(sampled.split(':').count(), 6);
        // Every window is pure ASCII, so all five digests are the digest of
        // SAMPLE_SIZE 'a' characters, and none of them is empty.
        let window = checksum(&"a".repeat(SAMPLE_SIZE)).unwrap();
        assert_eq!(
            sampled,
            format!(
                "{}:{}:{}:{}:{}:{}",
                DEFAULT_SAMPLE_LIMIT + 1,
                window,
                window,
                window,
                window,
                window
            )
        );
    }

    #[test]
    fn the_length_prefix_counts_utf16_code_units() {
        // 5000 code units, 4999 characters, 5002 UTF-8 bytes. Only the first
        // belongs in the prefix; the other two are what a bytes- or
        // chars-based port would emit.
        let content = "\u{65e5}\u{672c}\u{8a9e}";
        assert_eq!(content.encode_utf16().count(), 3);
        assert_eq!(content.chars().count(), 3);
        assert_eq!(content.len(), 9);
        let sampled = sampled_checksum_with_limit(content, 2).unwrap();
        assert!(sampled.starts_with("3:"), "prefix must be the code unit count");
        assert!(!sampled.starts_with("9:"));
    }

    #[test]
    fn a_non_bmp_character_counts_as_two_units_in_the_prefix() {
        // One character, two code units, four bytes.
        let sampled = sampled_checksum_with_limit("\u{1f389}", 1).expect("non empty");
        assert_eq!(sampled, "2:swncdy:swncdy:swncdy:swncdy:swncdy");
    }

    #[test]
    fn the_reference_sampling_of_a_known_string_is_reproduced() {
        // 5000 code units, one of which is a non-BMP character placed so that
        // the third window ends between its two surrogate halves.
        let content = format!("{}\u{1f389}{}", "a".repeat(4547), "b".repeat(451));
        assert_eq!(content.encode_utf16().count(), 5000);
        assert_eq!(content.chars().count(), 4999);
        assert_eq!(content.len(), 5002);
        assert_eq!(
            sampled_checksum_with_limit(&content, 10).as_deref(),
            Some("5000:1mtn83p:1mtn83p:1agkbfy:yy0emh:1mtn83p")
        );
    }

    #[test]
    fn a_window_that_ends_on_a_lone_surrogate_is_hashed_raw() {
        let content = format!("{}\u{1f389}{}", "a".repeat(4547), "b".repeat(451));
        // The String must outlive the borrows taken out of it.
        let sampled = sampled_checksum_with_limit(&content, 10).expect("non empty");
        let fields: Vec<&str> = sampled.split(':').collect();
        assert_eq!(fields.len(), 6);

        let plain = checksum(&"a".repeat(SAMPLE_SIZE)).unwrap();
        let split = fields[3];
        // The window really does stop on a lone high surrogate, so its digest
        // differs from an all-ASCII window of the same size.
        assert_ne!(split, plain);
        // And it is not the digest of the same window after a lossy round trip
        // through String, which would have replaced the lone surrogate with
        // U+FFFD. A port that rebuilt a String here would produce this value
        // instead.
        let lossy = checksum(&format!("{}\u{fffd}", "a".repeat(SAMPLE_SIZE - 1)))
            .expect("non empty");
        assert_ne!(split, lossy);
        // The following window contains both halves and differs from both.
        assert_ne!(fields[4], plain);
        assert_ne!(fields[4], split);
    }

    #[test]
    fn every_window_is_non_empty_even_when_the_content_is_shorter_than_a_window() {
        // The `?? ""` fallback in the source is unreachable; this pins why.
        for length in 1..=8usize {
            let content = "a".repeat(length);
            let sampled = sampled_checksum_with_limit(&content, 0).expect("non empty");
            let fields: Vec<&str> = sampled.split(':').collect();
            assert_eq!(fields.len(), 6, "length {length}");
            assert_eq!(fields[0], length.to_string());
            for field in &fields[1..] {
                assert!(!field.is_empty(), "length {length} produced an empty window");
                assert_eq!(*field, checksum(&content).unwrap());
            }
        }
        assert_eq!(
            sampled_checksum_with_limit("ab", 0).as_deref(),
            Some("2:lekqmi:lekqmi:lekqmi:lekqmi:lekqmi")
        );
    }

    #[test]
    fn the_zero_limit_still_produces_five_digests_for_a_one_unit_content() {
        assert_eq!(
            sampled_checksum_with_limit("a", 0).as_deref(),
            Some("1:1r9wi7g:1r9wi7g:1r9wi7g:1r9wi7g:1r9wi7g")
        );
        // A limit of 1 keeps a one-unit string in the short branch.
        assert_eq!(sampled_checksum_with_limit("a", 1), checksum("a"));
        assert_eq!(sampled_checksum_with_limit("abc", 3), checksum("abc"));
    }

    #[test]
    fn sampling_is_deterministic_and_depends_on_the_content() {
        let a = "a".repeat(600_000);
        let b = format!("{}b", "a".repeat(600_000));
        assert_eq!(sampled_checksum(&a), sampled_checksum(&a));
        assert_ne!(sampled_checksum(&a), sampled_checksum(&b));
        assert_eq!(sampled_checksum(&a).unwrap().split(':').count(), 6);
    }

    #[test]
    fn a_long_multi_byte_content_is_sampled_over_its_code_units() {
        // 1_000_000 code units, 1_000_000 characters, 2_000_000 UTF-8 bytes.
        // The prefix must be the code unit count, not the byte count.
        let content = "\u{e9}".repeat(1_000_000);
        assert_eq!(content.len(), 2_000_000);
        let sampled = sampled_checksum(&content).expect("non empty");
        assert!(sampled.starts_with("1000000:"), "got {sampled:.32}");
        assert_eq!(sampled.split(':').count(), 6);
        // Every window is 4096 identical code units, so all digests agree.
        let window = checksum(&"\u{e9}".repeat(SAMPLE_SIZE)).unwrap();
        assert_eq!(sampled, format!("1000000:{window}:{window}:{window}:{window}:{window}"));
    }
}
