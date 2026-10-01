//! Port of the portable part of `opencode/packages/core/src/tool/http-body.ts`.
//!
//! The Effect version streams an HTTP response into a buffer and fails when the
//! body is bigger than the cap. The streaming is not ported; the POLICY is,
//! and the policy is where the behaviour lives.
//!
//! Three decisions, in the order the source makes them:
//!
//! 1. The declared size comes from the `content-length` header, parsed with
//!    `Number.parseInt(h, 10)` - which parses a PREFIX and stops at the first
//!    character it does not understand. `"120abc"` declares 120, `"1e3"`
//!    declares 1, `"-5"` declares nothing (negative), `"abc"` declares
//!    nothing. That leniency is load-bearing: a header that lies about being a
//!    number is treated as absent rather than as zero.
//! 2. A declared size strictly above the cap fails BEFORE any byte is read.
//!    Equal to the cap is fine.
//! 3. The stream is the authority. Every chunk is counted as it arrives, an
//!    empty chunk is skipped without counting, and a chunk that would cross
//!    the cap fails even when the header said the body was small. So a lying
//!    header can only fail early, never smuggle a large body through.

use std::fmt;

/// The initial buffer size the TS allocates when the declared size is zero.
pub const DEFAULT_PREALLOCATION: u64 = 64 * 1024;

/// The size cap was exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooLarge {
    /// The cap that was crossed.
    pub maximum_bytes: u64,
    /// What was known when the failure was decided.
    pub observed_bytes: u64,
}

impl fmt::Display for TooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "response body exceeds {} bytes ({} observed)",
            self.maximum_bytes, self.observed_bytes
        )
    }
}

impl std::error::Error for TooLarge {}

/// `Number.parseInt(header, 10)`, reduced to the part the policy uses.
///
/// Returns `None` for anything that is not a non-negative safe integer, which
/// includes a missing header, garbage, a negative number, and the empty string.
/// A prefix is enough: `"120abc"` is 120.
pub fn declared_size(content_length: Option<&str>) -> Option<u64> {
    let raw = content_length?.trim_start();
    let bytes = raw.as_bytes();
    let mut end = 0usize;
    if matches!(bytes.first(), Some(b'+')) {
        end = 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return None; // NaN in JS terms
    }
    // `Number.isSafeInteger` rejects anything past 2^53 - 1.
    raw[digits_start..end].parse::<u64>().ok().filter(|n| *n <= MAX_SAFE_INTEGER)
}

/// The largest integer `Number.isSafeInteger` accepts.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// `collectBoundedResponseBody`, as a function over chunks.
///
/// Returns the body truncated to the bytes that were collected, or the first
/// size at which the cap was exceeded.
pub fn collect_bounded<I>(
    chunks: I,
    content_length: Option<&str>,
    maximum_bytes: u64,
) -> Result<Vec<u8>, TooLarge>
where
    I: IntoIterator<Item = Vec<u8>>,
{
    if let Some(declared) = declared_size(content_length) {
        if declared > maximum_bytes {
            return Err(TooLarge { maximum_bytes, observed_bytes: declared });
        }
    }
    let mut body: Vec<u8> = Vec::new();
    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }
        let size = body.len() as u64 + chunk.len() as u64;
        if size > maximum_bytes {
            return Err(TooLarge { maximum_bytes, observed_bytes: size });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(sizes: &[usize]) -> Vec<Vec<u8>> {
        sizes.iter().map(|n| vec![b'a'; *n]).collect()
    }

    #[test]
    fn a_clean_body_is_collected() {
        let out = collect_bounded(chunks(&[2, 3]), Some("5"), 10).unwrap();
        assert_eq!(out, vec![b'a'; 5]);
    }

    #[test]
    fn a_declared_size_above_the_cap_fails_before_reading_anything() {
        let err = collect_bounded(Vec::new(), Some("11"), 10).unwrap_err();
        assert_eq!(err, TooLarge { maximum_bytes: 10, observed_bytes: 11 });
    }

    #[test]
    fn a_declared_size_equal_to_the_cap_is_accepted() {
        assert!(collect_bounded(chunks(&[10]), Some("10"), 10).is_ok());
    }

    #[test]
    fn the_stream_overrules_a_small_declared_size() {
        // The header says 2, the stream sends 20 bytes: the cap is what counts.
        let err = collect_bounded(chunks(&[20]), Some("2"), 10).unwrap_err();
        assert_eq!(err.observed_bytes, 20);
    }

    #[test]
    fn a_body_exactly_at_the_cap_is_collected() {
        let out = collect_bounded(chunks(&[4, 6]), Some("10"), 10).unwrap();
        assert_eq!(out.len(), 10);
    }

    #[test]
    fn one_byte_past_the_cap_fails() {
        let err = collect_bounded(chunks(&[4, 7]), Some("10"), 10).unwrap_err();
        assert_eq!(err.observed_bytes, 11);
    }

    #[test]
    fn empty_chunks_are_skipped_and_do_not_count() {
        let out = collect_bounded(chunks(&[0, 3, 0, 0, 2]), Some("5"), 5).unwrap();
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn a_missing_header_defers_everything_to_the_stream() {
        assert!(collect_bounded(chunks(&[5]), None, 5).is_ok());
        assert!(collect_bounded(chunks(&[6]), None, 5).is_err());
    }

    #[test]
    fn parse_int_reads_a_prefix_and_stops() {
        assert_eq!(declared_size(Some("120abc")), Some(120));
        assert_eq!(declared_size(Some("1e3")), Some(1), "exponent notation stops after 1");
        assert_eq!(declared_size(Some("  42")), Some(42), "leading spaces are skipped");
        assert_eq!(declared_size(Some("+7")), Some(7));
    }

    #[test]
    fn a_header_that_is_not_a_number_declares_nothing() {
        assert_eq!(declared_size(None), None);
        assert_eq!(declared_size(Some("")), None);
        assert_eq!(declared_size(Some("abc")), None);
        assert_eq!(declared_size(Some("-5")), None, "negative is rejected");
        assert_eq!(declared_size(Some("1.5")), Some(1), "the fraction is dropped, not rounded");
    }

    #[test]
    fn a_zero_length_header_is_a_valid_declared_size() {
        assert_eq!(declared_size(Some("0")), Some(0));
        // The TS pre-allocates `declared || 64 KiB`, so zero means "no
        // declaration of size" for the buffer only, never a cap of zero.
        let out = collect_bounded(chunks(&[3]), Some("0"), 3).unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn an_unsafe_integer_is_treated_as_absent() {
        let huge = (MAX_SAFE_INTEGER + 1).to_string();
        assert_eq!(declared_size(Some(&huge)), None);
        // ...and with no usable declaration the stream decides, so a 9000
        // petabyte claim cannot fail the request early.
        assert!(collect_bounded(chunks(&[1]), Some(&huge), 10).is_ok());
    }

    #[test]
    fn the_preallocation_constant_is_the_source_value() {
        assert_eq!(DEFAULT_PREALLOCATION, 65_536);
    }

    #[test]
    fn the_error_names_the_cap_it_crossed() {
        let err = collect_bounded(chunks(&[11]), None, 10).unwrap_err();
        assert_eq!(err.to_string(), "response body exceeds 10 bytes (11 observed)");
    }
}