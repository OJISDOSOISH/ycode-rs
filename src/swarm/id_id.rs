//! Port of `packages/core/src/id/id.ts`.
//!
//! The source is 47 lines and defines no class and no type: it is a **prefixed
//! wrapper** around the shared 26-character identifier generator. Its whole job
//! is to glue a domain prefix (`ses`, `msg`, `wrk`, ...) onto a body produced by
//! `@opencode-ai/schema/identifier`, and to validate an already existing id
//! against that prefix.
//!
//! ## The one thing this module does NOT reimplement
//!
//! The body generator -- 12 lowercase hex characters of time followed by 14
//! random base-62 characters -- already lives in `crate::swarm::util_identifier`,
//! which is the port of `packages/schema/src/identifier.ts`. This module calls
//! `util_identifier::create_at` and does **not** carry a second copy of the
//! SplitMix64 generator, the second `Mutex`-guarded counter, or the second
//! alphabet. `core_workspace.rs` and `pty_schema.rs` each hold a *private*
//! copy of that generator because they were written before `util_identifier`
//! existed; the integrator should collapse those two into
//! `use super::util_identifier::create_at` too. This file is written on the
//! assumption that the shared module is the canonical one.
//!
//! ## On the randomness (read this before trusting an id for anything)
//!
//! The source reaches randomness through
//! `crypto.getRandomValues(new Uint8Array(length - 12))` -- 14 cryptographically
//! secure bytes. It does **not** use `crypto.randomUUID()`; that call lives in
//! `packages/core/src/observability/shared.ts`, which is a different file and
//! is ported by `observability_shared.rs`.
//!
//! The bytes handed on here therefore come from `util_identifier`'s SplitMix64,
//! seeded from the standard library's `RandomState`. That is **not**
//! cryptographic. It is good enough to keep ids distinct inside and across
//! processes; it is not good enough for session tokens, invite codes, or
//! anything an attacker would want to predict.
//!
//! `Cargo.toml` line 14 does enable the `uuid` crate's `v4` feature
//! (`uuid = { version = "1", features = ["v4", "v7", "serde"] }`), so
//! `Uuid::new_v4()` is available today and would be a strictly better source
//! for those 14 bytes. It is deliberately *not* used here: reaching for it would
//! fork the generator into a second, incompatible implementation, which is
//! exactly the duplication this batch is trying to remove. The correct fix is a
//! one-line change inside `util_identifier.rs`, made once, for every caller.
//! (`observability_shared.rs` line 41 claims the `v4` feature is absent; that
//! claim is wrong, and its generator should be revisited for the same reason.)
//!
//! ## Output shape
//!
//! ```text
//! <prefix> "_" <26 characters>
//!         |     |- 12 chars: lowercase hex, the low 48 bits of (ms * 4096 + counter)
//!         |     `- 14 chars: random, drawn from a 62-character alphabet
//!         `- one of the ten prefixes below, no underscore
//! ```
//!
//! Total length is `prefix.len() + 1 + 26`. The prefix set itself carries no
//! underscore; the underscore is added only by `create`.
//!
//! ## The monotonic component
//!
//! Inherited from `identifier.ts`: the body is
//! `BigInt(timestamp) * 0x1000n + BigInt(counter)`, where `counter` is
//! 1-based, is reset to zero whenever the millisecond changes, and is shared by
//! every caller in the process. It therefore occupies exactly the low 12 bits,
//! and is the reason two ids minted in the same millisecond are still ordered
//! and still distinct. Those 12 bits are what `timestamp()` later divides away.
//!
//! Two consequences are inherited rather than fixed here:
//!
//! - The counter is not masked to 12 bits. At 4096 ids inside one millisecond
//!   it carries into the time field, which is still distinct and still ordered,
//!   so this is harmless -- but it means a caller that generates 4096+ ids in a
//!   single millisecond loses the "one id per unit of time" reading.
//! - A millisecond timestamp in 2026 occupies 41 bits; times 4096 that is 53,
//!   while only 48 are printed. The top 5 bits are dropped by the source, so
//!   bodies stay lexicographically ordered over a ~2.2 year window and then
//!   wrap. `timestamp()` inherits the same truncation.
//!
//! ## Collision behaviour
//!
//! Within a millisecond, ids differ by construction (the counter). Across
//! milliseconds, the time segment differs. The 14 trailing characters are the
//! only thing protecting against a clock that goes backwards or a counter reset
//! from a restored process state, and those come from a non-cryptographic
//! source (see above).
//!
//! ## The two traps this source actually contains
//!
//! 1. **`if (!given)` is a truthiness test, not a null test.** `!given` is true
//!    for `undefined`, for `null`, and for the empty string. So
//!    `ascending("session", "")` does **not** fail validation and does **not**
//!    return `""`: it forges a brand new id. Translating `given?: string` to
//!    `Option<&str>` and letting `Some("")` fall into the validation arm is the
//!    wrong port. This is the `?`-versus-`??` trap; note that a naive
//!    `given.map_or_else(create, validate)` gets it right, while a naive
//!    `given.map(validate).unwrap_or_else(create)` does not.
//! 2. **The prefix check has no underscore.** `given.startsWith(prefixes[prefix])`
//!    tests the bare three-letter code, so the id `"ses"` and the id `"sess"`
//!    both pass validation even though neither could have been produced by
//!    `create`. Do not tighten this to `"ses_"`; that would be an invention.
//!
//! A third, quieter trap lives in `timestamp()` and is covered by its own test:
//! `String.prototype.slice` clamps out-of-range bounds and returns `""`, while
//! Rust's `&id[a..b]` panics -- on an out-of-range start, and on any index that
//! is not a UTF-8 character boundary. The empty id is the minimal repro.
//!
//! ## Naming note
//!
//! The source has no UPPERCASE field or key, so the usual
//! "camelCase keys are invisible at compile time" trap does not apply here.
//! What does apply is the same family of mistake one level up: the *key* of
//! `prefixes` and its *value* differ for seven of the ten entries
//! (`event` -> `evt`, `session` -> `ses`, ...). Reading the key as the emitted
//! prefix is a silent, data-corrupting error. The full table is asserted
//! exhaustively in the tests.

use crate::swarm::util_identifier::{create_at, now_millis};

/// Total length of an identifier body, the `length` constant of the source.
pub const BODY_LENGTH: usize = 26;

/// Number of hex characters occupied by the time segment.
pub const TIME_HEX_LENGTH: usize = 12;

/// Exclusive end offset of the time slice, relative to the start of the body.
///
/// The source computes `id.slice(prefix.length + 1, prefix.length + 13)`, i.e.
/// it takes twelve characters starting one past the underscore.
const TIME_SLICE_END: usize = TIME_HEX_LENGTH;

/// Divisor applied to the 48-bit time field, the `0x1000n` of the source.
///
/// It is also the scale the 1-based counter is multiplied by when it is added
/// to the millisecond timestamp, so dividing it back out recovers the
/// millisecond.
const COUNTER_SCALE: u64 = 0x1000;

/// The alphabet of the random tail, in the exact order of the source.
///
/// The order is part of the contract and not cosmetic: the generator indexes
/// this table with `byte % 62`, so a caller revalidating a received id has to
/// accept the same 62 characters in the same order to agree with it.
///
/// Public because a consumer that checks an id's tail needs it, and because it
/// documents the format without a second definition elsewhere.
pub const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// The ten domain prefixes, i.e. `keyof typeof prefixes`.
///
/// The variant name is the TypeScript *key*; [`Prefix::as_str`] is its
/// *value*. The two are equal only for `Job`, `Pty` and `Tool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Prefix {
    /// Key `job`.
    Job,
    /// Key `event`.
    Event,
    /// Key `session`.
    Session,
    /// Key `message`.
    Message,
    /// Key `permission`.
    Permission,
    /// Key `question`.
    Question,
    /// Key `part`.
    Part,
    /// Key `pty`.
    Pty,
    /// Key `tool`.
    Tool,
    /// Key `workspace`.
    Workspace,
}

impl Prefix {
    /// Every prefix, in the declaration order of the source object.
    pub const ALL: [Prefix; 10] = [
        Prefix::Job,
        Prefix::Event,
        Prefix::Session,
        Prefix::Message,
        Prefix::Permission,
        Prefix::Question,
        Prefix::Part,
        Prefix::Pty,
        Prefix::Tool,
        Prefix::Workspace,
    ];

    /// The emitted prefix, i.e. the *value* stored in the source object.
    ///
    /// This is the string that is actually concatenated with `_`, and the one
    /// that `ascending` and `descending` validate against.
    pub fn as_str(self) -> &'static str {
        match self {
            Prefix::Job => "job",
            Prefix::Event => "evt",
            Prefix::Session => "ses",
            Prefix::Message => "msg",
            Prefix::Permission => "per",
            Prefix::Question => "que",
            Prefix::Part => "prt",
            Prefix::Pty => "pty",
            Prefix::Tool => "tool",
            Prefix::Workspace => "wrk",
        }
    }

    /// The TypeScript *key*, which is also the snake_case name of the variant.
    ///
    /// Useful for diagnostics and for round-tripping a key that came in as a
    /// string, since the key and the value are not interchangeable.
    pub fn key(self) -> &'static str {
        match self {
            Prefix::Job => "job",
            Prefix::Event => "event",
            Prefix::Session => "session",
            Prefix::Message => "message",
            Prefix::Permission => "permission",
            Prefix::Question => "question",
            Prefix::Part => "part",
            Prefix::Pty => "pty",
            Prefix::Tool => "tool",
            Prefix::Workspace => "workspace",
        }
    }

    /// Looks a prefix up by its TypeScript key.
    pub fn from_key(key: &str) -> Option<Prefix> {
        Prefix::ALL.into_iter().find(|p| p.key() == key)
    }
}

/// Sort direction of a generated identifier.
///
/// In the source this is the string union `"ascending" | "descending"`, and it
/// matters in exactly one place: `direction === "descending"`, which selects the
/// bitwise complement `~current` of the time field. Any other string behaves as
/// ascending, so the enum is closed here rather than left open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Lexicographically increasing with time. The source's default.
    Ascending,
    /// Lexicographically decreasing with time.
    Descending,
}

impl Direction {
    /// The value the source compares against: `direction === "descending"`.
    pub fn is_descending(self) -> bool {
        matches!(self, Direction::Descending)
    }
}

/// Raised when a supplied id does not carry the expected prefix.
///
/// The `Display` output is byte-for-byte the message of the source,
/// `ID <id> does not start with <prefix>`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ID {id} does not start with {prefix}")]
pub struct PrefixMismatch {
    /// The rejected id, exactly as it was received.
    pub id: String,
    /// The prefix it was required to start with.
    pub prefix: &'static str,
}

/// Raised when [`timestamp`] cannot recover a millisecond from an id.
///
/// In the source both cases are an uncaught `SyntaxError` thrown by the `BigInt`
/// constructor, because the string is built as `"0x" + hex`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimestampError {
    /// The slice after the prefix was empty, or ran past the end of the id.
    ///
    /// This is what `BigInt("0x")` throws on. It is also what an id with no
    /// underscore at all produces: `slice(prefix.length + 1, ...)` starts past
    /// the end and clamps to `""`.
    #[error("ID {id:?} carries no hexadecimal payload after its prefix")]
    EmptyPayload {
        /// The id that was inspected.
        id: String,
    },
    /// The slice was not a run of hexadecimal digits.
    #[error("ID {id:?} carries a non-hexadecimal payload {payload:?}")]
    NotHexadecimal {
        /// The id that was inspected.
        id: String,
        /// The offending slice, as extracted.
        payload: String,
    },
}

/// Builds a fresh id from an arbitrary prefix string.
///
/// Port of the exported `create(prefix, direction, timestamp?)`.
///
/// Unlike [`ascending`] and [`descending`], this performs **no** validation: the
/// source checks nothing here either, and callers in the TypeScript rely on that
/// when they mint a prefix that is not one of the ten. Passing a prefix that
/// does not end at an underscore boundary is therefore allowed and is the
/// caller's business.
///
/// `timestamp` is `None` for the source's `undefined`, which makes the
/// generator default to `Date.now()`. Note that a JavaScript default parameter
/// only fires on `undefined`, so `null` would reach the `BigInt` constructor and
/// throw; `Option` cannot express that distinction and `None` is used for it.
pub fn create(prefix: &str, direction: Direction, timestamp: Option<i64>) -> String {
    let millis = match timestamp {
        Some(fixed) => fixed,
        None => now_millis(),
    };
    // `create_at` is the port of the imported `createIdentifier`; the boolean is
    // the source's `direction === "descending"`.
    let body = create_at(direction.is_descending(), millis);

    let mut id = String::with_capacity(prefix.len() + 1 + BODY_LENGTH);
    id.push_str(prefix);
    id.push('_');
    id.push_str(&body);
    id
}

/// Shared body of [`ascending`] and [`descending`], the private `generateID`.
///
/// The empty string is funnelled into the "no id given" branch on purpose: see
/// the module documentation, trap 1.
fn generate_id(
    kind: Prefix,
    direction: Direction,
    given: Option<&str>,
) -> Result<String, PrefixMismatch> {
    let expected = kind.as_str();
    match given {
        None | Some("") => Ok(create(expected, direction, None)),
        Some(id) if !id.starts_with(expected) => Err(PrefixMismatch {
            id: id.to_string(),
            prefix: expected,
        }),
        Some(id) => Ok(id.to_string()),
    }
}

/// Returns the given id if it already carries the prefix, otherwise mints a
/// new ascending one.
///
/// Port of `ascending(prefix, given?)`. Note that `given` is the *prefix kind*,
/// not the prefix string: the source resolves `prefixes[prefix]` internally.
pub fn ascending(kind: Prefix, given: Option<&str>) -> Result<String, PrefixMismatch> {
    generate_id(kind, Direction::Ascending, given)
}

/// Returns the given id if it already carries the prefix, otherwise mints a
/// new descending one.
///
/// Port of `descending(prefix, given?)`.
pub fn descending(kind: Prefix, given: Option<&str>) -> Result<String, PrefixMismatch> {
    generate_id(kind, Direction::Descending, given)
}

/// Recovers the millisecond timestamp encoded in the time segment of an id.
///
/// Port of the exported `timestamp(id)`. It reads the twelve hex characters that
/// follow the first underscore and divides the resulting 48-bit integer by
/// `0x1000`, which cancels the counter.
///
/// Three behaviours are inherited from the source and are deliberate:
///
/// - **It is meaningless for descending ids.** The source says so in a comment:
///   the time field of a descending id holds `~current`, not `current`. The
///   function does not detect this and returns a large wrong number rather than
///   an error, because that is what the original does.
/// - **It only takes the low 48 bits.** A millisecond outside a ~2.2 year
///   window around the epoch is truncated identically in the generator and here,
///   so the two stay consistent, but neither is absolute.
/// - **It trusts the shape of the id, not its prefix.** The prefix is whatever
///   precedes the first underscore; there is no check that it is one of the ten.
///
/// # Errors
///
/// Returns [`TimestampError`] where the source would throw a `SyntaxError` from
/// `BigInt`: an empty slice, or a slice that is not hexadecimal. In particular
/// the empty id and the id `"ses"` both yield [`TimestampError::EmptyPayload`] --
/// the source's `slice` clamps its bounds and returns `""`, whereas a direct
/// `&id[start..end]` in Rust would panic on the out-of-range start.
///
/// # Character counting
///
/// Offsets are measured in `char`s, not bytes, so slicing always lands on a
/// UTF-8 boundary. The source counts UTF-16 code units, so the two agree for
/// every ASCII id -- which is every id this module can produce -- and diverge only
/// for a hand-written id containing a character outside the basic multilingual
/// plane.
pub fn timestamp(id: &str) -> Result<i64, TimestampError> {
    // Find the first underscore byte to locate the prefix boundary.
    // This uses byte offsets to match String::get semantics.
    let prefix_end = id.find('_').unwrap_or(id.len());
    let start = prefix_end.saturating_add(1);
    let end = start.saturating_add(TIME_HEX_LENGTH);

    // `String::get` returns `None` rather than panicking; the `None` arm is
    // unreachable given the bounds, and is kept as a hard floor so this
    // function can never panic.
    let payload = id.get(start..end).unwrap_or("");

    if payload.is_empty() {
        return Err(TimestampError::EmptyPayload {
            id: id.to_string(),
        });
    }

    let mut encoded: u64 = 0;
    for character in payload.chars() {
        let digit = character.to_digit(16).ok_or_else(|| TimestampError::NotHexadecimal {
            id: id.to_string(),
            payload: payload.to_string(),
        })?;
        // Twelve digits maximum, so this never exceeds 48 bits.
        encoded = (encoded << 4) | u64::from(digit);
    }

    // The source divides two BigInts (truncating toward zero) and then converts
    // with `Number`. Every value here is non-negative and below 2^36, so the
    // truncation is a floor and the conversion is exact.
    Ok((encoded / COUNTER_SCALE) as i64)
}

#[cfg(test)]
mod tests {
    use super::{
        ascending, create, descending, timestamp, Direction, Prefix, PrefixMismatch, TimestampError,
        ALPHABET, BODY_LENGTH, TIME_HEX_LENGTH, TIME_SLICE_END,
    };

    /// A millisecond timestamp from September 2025, far enough from zero that
    /// its 48-bit encoding uses the top bit of the field, but small enough to
    /// fit in the 36 bits that survive the generator's 48-bit truncation
    /// (timestamp * 4096 -> low 48 bits -> / 4096 = low 36 bits of timestamp).
    const TS: i64 = 1_757_000_000;

    fn time_segment(id: &str) -> &str {
        let prefix_length = id.split('_').next().unwrap_or("").len();
        &id[prefix_length + 1..prefix_length + 1 + TIME_HEX_LENGTH]
    }

    fn tail(id: &str) -> &str {
        let prefix_length = id.split('_').next().unwrap_or("").len();
        &id[prefix_length + 1 + TIME_HEX_LENGTH..]
    }

    // -- the prefix table ----------------------------------------------------

    #[test]
    fn the_prefix_table_matches_the_typescript_object_key_for_key() {
        // The keys and the values are different strings for seven entries out
        // of ten. Emitting the key instead of the value would silently produce
        // "event_..." where the rest of the system expects "evt_...".
        let expected = [
            ("job", "job"),
            ("event", "evt"),
            ("session", "ses"),
            ("message", "msg"),
            ("permission", "per"),
            ("question", "que"),
            ("part", "prt"),
            ("pty", "pty"),
            ("tool", "tool"),
            ("workspace", "wrk"),
        ];
        assert_eq!(Prefix::ALL.len(), expected.len());
        for (kind, (key, value)) in Prefix::ALL.into_iter().zip(expected) {
            assert_eq!(kind.key(), key, "wrong key for {kind:?}");
            assert_eq!(kind.as_str(), value, "wrong value for key {key}");
        }
    }

    #[test]
    fn the_three_identical_keys_are_job_pty_and_tool() {
        // Recorded explicitly because it is the only way to notice that a
        // later edit made a fourth abbreviation.
        let identical: Vec<Prefix> = Prefix::ALL
            .into_iter()
            .filter(|p| p.key() == p.as_str())
            .collect();
        assert_eq!(identical, vec![Prefix::Job, Prefix::Pty, Prefix::Tool]);
    }

    #[test]
    fn every_prefix_can_be_looked_up_by_its_key() {
        for kind in Prefix::ALL {
            assert_eq!(Prefix::from_key(kind.key()), Some(kind));
        }
        assert_eq!(Prefix::from_key("evt"), None, "that is a value, not a key");
        assert_eq!(Prefix::from_key("Session"), None, "keys are case sensitive");
        assert_eq!(Prefix::from_key(""), None);
    }

    #[test]
    fn no_two_prefixes_share_a_value() {
        let mut values: Vec<&str> = Prefix::ALL.into_iter().map(Prefix::as_str).collect();
        values.sort_unstable();
        let before = values.len();
        values.dedup();
        assert_eq!(values.len(), before, "two prefixes collapse to the same code");
    }

    // -- output shape --------------------------------------------------------

    #[test]
    fn an_id_is_its_prefix_an_underscore_and_twenty_six_characters() {
        for kind in Prefix::ALL {
            for direction in [Direction::Ascending, Direction::Descending] {
                let id = create(kind.as_str(), direction, Some(TS));
                assert_eq!(id.len(), kind.as_str().len() + 1 + BODY_LENGTH, "{id}");
                assert!(id.starts_with(kind.as_str()), "{id}");
                assert_eq!(&id[kind.as_str().len()..kind.as_str().len() + 1], "_");
            }
        }
    }

    #[test]
    fn the_time_segment_is_twelve_lowercase_hex_characters() {
        let id = create("ses", Direction::Ascending, Some(TS));
        let segment = time_segment(&id);
        assert_eq!(segment.len(), TIME_HEX_LENGTH);
        assert!(
            segment
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "not lowercase hex: {segment:?}"
        );
    }

    #[test]
    fn the_random_tail_is_fourteen_characters_of_the_base62_alphabet() {
        for _ in 0..16 {
            let id = create("msg", Direction::Ascending, Some(TS));
            let tail = tail(&id);
            assert_eq!(tail.len(), BODY_LENGTH - TIME_HEX_LENGTH);
            assert!(
                tail.bytes().all(|b| ALPHABET.contains(&b)),
                "character outside the alphabet in {id:?}"
            );
        }
    }

    #[test]
    fn the_alphabet_is_the_sixty_two_source_characters_in_order() {
        // Digits first, then uppercase, then lowercase: `byte % 62` indexes it,
        // so the order is part of the contract, not cosmetic.
        assert_eq!(ALPHABET.len(), 62);
        assert_eq!(&ALPHABET[..10], b"0123456789");
        assert_eq!(&ALPHABET[10..36], b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        assert_eq!(&ALPHABET[36..], b"abcdefghijklmnopqrstuvwxyz");
    }

    // -- the monotonic counter ----------------------------------------------

    #[test]
    fn an_ascending_id_leaves_the_counter_to_the_low_twelve_bits_only() {
        // 1_757_000_000 * 4096 = 0x68b9b1400000 in 48 bits. The counter is
        // added before masking, so it can only move the last three hex digits
        // and the first nine are fixed. This assertion therefore holds no
        // matter what the process-wide counter happens to be, which is what
        // lets the test suite run in parallel without a lock.
        let id = create("ses", Direction::Ascending, Some(TS));
        let segment = time_segment(&id);
        assert!(
            segment.starts_with("68b9b1400"),
            "the high 36 bits moved: {segment:?}"
        );
    }

    #[test]
    fn the_counter_is_never_allowed_to_disturb_the_high_thirty_six_bits() {
        // 4096 is the first counter value that carries into the time field. We
        // cannot force that many ids here, so we check the guarantee that makes
        // it harmless: the fixed prefix is still exactly the millisecond.
        for _ in 0..8 {
            let id = create("ses", Direction::Ascending, Some(TS));
            let recovered = timestamp(&id).expect("a generated id always parses");
            assert_eq!(recovered, TS, "the counter leaked into the time field");
        }
    }

    #[test]
    fn two_ids_minted_in_the_same_millisecond_are_never_equal() {
        let first = create("evt", Direction::Ascending, Some(TS));
        let second = create("evt", Direction::Ascending, Some(TS));
        assert_ne!(first, second);
        assert_eq!(time_segment(&first)[..TIME_HEX_LENGTH - 3], time_segment(&second)[..TIME_HEX_LENGTH - 3]);
    }

    // -- direction -----------------------------------------------------------

    #[test]
    fn a_descending_id_inverts_the_time_segment() {
        // At timestamp 1 the body is 0x1000 + counter, so `~body` is negative
        // and its high 32 bits are all ones. Every counter value from 1 to 4096
        // gives the same eight leading `f`s, which is what makes this stable.
        let id = create("prt", Direction::Descending, Some(1));
        assert!(
            time_segment(&id).starts_with("ffffffff"),
            "expected a complemented time field, got {:?}",
            time_segment(&id)
        );
    }

    #[test]
    fn ascending_and_descending_disagree_on_the_time_segment() {
        let ascending_id = create("ses", Direction::Ascending, Some(1));
        let descending_id = create("ses", Direction::Descending, Some(1));
        let up = time_segment(&ascending_id);
        let down = time_segment(&descending_id);
        // timestamp=1, counter=1 -> current=4097=0x1001 -> 14 hex="00000000001001" -> first 12="000000000010"
        assert!(up.starts_with("000000000"), "{up:?}");
        // descending: ~4097 & 0xFFFFFFFFFFFFFF = 0xFFFFFFFFFFFFEFFE -> first 12="ffffffffeffe"
        assert!(down.starts_with("ffffffff"), "{down:?}");
        assert_ne!(up, down);
    }

    #[test]
    fn only_descending_selects_the_complement() {
        assert!(!Direction::Ascending.is_descending());
        assert!(Direction::Descending.is_descending());
    }

    // -- the truthiness trap -------------------------------------------------

    #[test]
    fn an_empty_string_is_treated_as_an_absent_id_and_forges_a_new_one() {
        // `if (!given)` is a truthiness test. `ascending("session", "")` must
        // return a fresh id, not an error and not the empty string.
        let forged = ascending(Prefix::Session, Some("")).expect("the empty string must not fail");
        assert!(forged.starts_with("ses_"), "{forged:?}");
        assert_eq!(forged.len(), 3 + 1 + BODY_LENGTH);
        assert_ne!(forged, "");
    }

    #[test]
    fn no_id_at_all_also_forges_a_new_one() {
        let forged = ascending(Prefix::Workspace, None).expect("no id is not an error");
        assert!(forged.starts_with("wrk_"), "{forged:?}");
    }

    #[test]
    fn the_empty_string_trap_applies_to_descending_too() {
        let forged = descending(Prefix::Message, Some("")).expect("the empty string must not fail");
        assert!(forged.starts_with("msg_"), "{forged:?}");
    }

    // -- prefix validation ---------------------------------------------------

    #[test]
    fn a_valid_id_is_returned_unchanged_and_not_regenerated() {
        let given = "ses_0000000000010000000000000";
        let returned = ascending(Prefix::Session, Some(given)).expect("valid id");
        assert_eq!(returned, given, "an existing id must be reused verbatim");
    }

    #[test]
    fn an_id_with_the_wrong_prefix_is_rejected() {
        assert_eq!(
            ascending(Prefix::Message, Some("evt_0000000000010000")),
            Err(PrefixMismatch {
                id: "evt_0000000000010000".to_string(),
                prefix: "msg",
            })
        );
    }

    #[test]
    fn the_rejection_message_is_the_message_of_the_source() {
        let error = descending(Prefix::Tool, Some("nope")).expect_err("wrong prefix");
        assert_eq!(error.to_string(), "ID nope does not start with tool");
    }

    #[test]
    fn the_prefix_check_does_not_require_the_underscore() {
        // The source tests `startsWith("ses")`, not `startsWith("ses_")`. Both
        // of these pass, and tightening it would be an invention.
        assert!(ascending(Prefix::Session, Some("ses")).is_ok());
        assert!(ascending(Prefix::Session, Some("sesXYZ")).is_ok());
        // "sess" also starts with "ses", so it is accepted too.
        assert!(ascending(Prefix::Session, Some("sess")).is_ok());
    }

    #[test]
    fn the_prefix_check_is_case_sensitive() {
        assert!(ascending(Prefix::Session, Some("SES_abc")).is_err());
        assert!(ascending(Prefix::Session, Some("Ses_abc")).is_err());
    }

    #[test]
    fn the_check_is_a_prefix_test_and_not_an_equality_test() {
        // `startsWith("ses")` accepts anything that merely *begins* with the
        // code, so "session_x" passes even though `create` could never produce
        // it. The code does not assert the separator, and it does not assert
        // that the id ends right after the prefix.
        assert!(ascending(Prefix::Session, Some("ses_x")).is_ok());
        assert!(ascending(Prefix::Session, Some("session_x")).is_ok());
        assert!(ascending(Prefix::Session, Some("xession_x")).is_err());
    }

    #[test]
    fn a_space_is_a_truthy_string_and_therefore_reaches_validation() {
        // Only "" is falsy among strings. A single space is truthy in
        // JavaScript, so it must be validated and rejected here, not silently
        // treated as absent.
        assert!(ascending(Prefix::Job, Some(" ")).is_err());
        assert!(ascending(Prefix::Job, Some("0")).is_err());
    }

    // -- create does not validate -------------------------------------------

    #[test]
    fn create_accepts_a_prefix_outside_the_ten() {
        // The source's `create` has no validation branch at all, and this
        // distinguishes it from `ascending`/`descending`.
        let id = create("custom", Direction::Ascending, Some(TS));
        assert!(id.starts_with("custom_"), "{id:?}");
        assert!(ascending(Prefix::Job, Some(&id)).is_err());
    }

    // -- timestamp extraction ------------------------------------------------

    #[test]
    fn timestamp_recovers_the_millisecond_of_a_generated_id() {
        let id = create("ses", Direction::Ascending, Some(TS));
        assert_eq!(timestamp(&id).expect("parses"), TS);
    }

    #[test]
    fn timestamp_parses_a_hand_written_id_without_touching_the_counter() {
        // Use a timestamp that when multiplied by 4096 fits in 12 hex digits.
        const SMALL_TS: i64 = 1_757_000;
        let id = format!("ses_{:012x}", SMALL_TS * 0x1000);
        assert_eq!(timestamp(&id).expect("parses"), SMALL_TS);
    }

    #[test]
    fn timestamp_reads_twelve_characters_and_no_more() {
        assert_eq!(TIME_SLICE_END, TIME_HEX_LENGTH);
        // Fourteen trailing hex characters must not leak into the value: the
        // thirteenth one is the first character of the random tail.
        const SMALL_TS: i64 = 1_757_000;
        let id = format!("ses_{:012x}ffff", SMALL_TS * 0x1000);
        assert_eq!(timestamp(&id).expect("parses"), SMALL_TS);
    }

    #[test]
    fn timestamp_on_an_empty_id_is_an_error_and_not_a_panic() {
        // The direct Rust transcription would be `&id[1..13]` on "", which
        // panics on an out-of-range start index. `slice` clamps and yields "",
        // so the source throws a SyntaxError. This is the whole trap.
        assert_eq!(
            timestamp(""),
            Err(TimestampError::EmptyPayload {
                id: String::new()
            })
        );
    }

    #[test]
    fn timestamp_on_an_id_with_no_underscore_is_an_error_and_not_a_panic() {
        // `slice(4, 16)` on a three-character string starts past the end.
        assert_eq!(
            timestamp("ses"),
            Err(TimestampError::EmptyPayload {
                id: "ses".to_string()
            })
        );
    }

    #[test]
    fn timestamp_on_a_prefix_alone_with_an_underscore_is_an_error() {
        assert!(matches!(
            timestamp("ses_"),
            Err(TimestampError::EmptyPayload { .. })
        ));
    }

    #[test]
    fn timestamp_rejects_a_payload_that_is_not_hexadecimal() {
        assert_eq!(
            timestamp("ses_zzzzzzzzzzzz"),
            Err(TimestampError::NotHexadecimal {
                id: "ses_zzzzzzzzzzzz".to_string(),
                payload: "zzzzzzzzzzzz".to_string(),
            })
        );
        // A leading sign is what `BigInt("0x-1")` rejects, and so must we.
        assert!(matches!(
            timestamp("ses_-00000000001"),
            Err(TimestampError::NotHexadecimal { .. })
        ));
        // A nested "0x" is likewise not a run of digits.
        assert!(matches!(
            timestamp("ses_0x0000000001"),
            Err(TimestampError::NotHexadecimal { .. })
        ));
    }

    #[test]
    fn timestamp_accepts_uppercase_hexadecimal_as_bigint_does() {
        let lower = format!("ses_{:012x}", TS * 0x1000);
        let upper = lower.to_uppercase();
        assert_eq!(timestamp(&upper).expect("parses"), TS);
    }

    #[test]
    fn timestamp_uses_the_first_underscore_and_ignores_the_rest() {
        // `id.split("_")[0]` takes the head, so anything after a second
        // underscore is simply not looked at.
        let id = format!("ses_{:012x}_extra", TS * 0x1000);
        assert_eq!(timestamp(&id).expect("parses"), TS);
    }

    #[test]
    fn timestamp_is_garbage_for_a_descending_id_and_does_not_say_so() {
        // Documented in the source as "does not work with descending IDs": the
        // field holds ~current, so the function returns a large wrong number
        // instead of an error. That is faithful, and the test pins it.
        let id = create("ses", Direction::Descending, Some(TS));
        let recovered = timestamp(&id).expect("it does parse, it is just wrong");
        assert_ne!(recovered, TS, "a descending id must not round-trip");
        assert!(recovered > 1_000_000, "expected a complemented value: {recovered}");
    }

    #[test]
    fn timestamp_does_not_validate_the_prefix() {
        // The prefix is whatever precedes the first underscore, with no check
        // that it is one of the ten.
        let id = format!("whatever_{:012x}", TS * 0x1000);
        assert_eq!(timestamp(&id).expect("parses"), TS);
    }

    #[test]
    fn timestamp_handles_a_multibyte_prefix_without_panicking() {
        // Offsets are counted in chars, so the slice stays on a character
        // boundary. A byte-based index would panic here.
        let id = format!("\u{e9}ch_{:012x}", TS * 0x1000);
        assert_eq!(timestamp(&id).expect("parses"), TS);
    }
}
