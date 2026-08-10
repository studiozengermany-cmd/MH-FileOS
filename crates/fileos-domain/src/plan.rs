//! Immutable action-plan domain for EP-007 slice A.
//!
//! # Safety posture
//!
//! This module performs **no filesystem access**. It models a proposal only. It never renames,
//! copies, deletes, or quarantines anything. All mutation remains forbidden until a later, gated
//! executor use case exists (see `docs/SAFETY-INVARIANTS.md` and `PLANS.md` EP-007). These types
//! let a plan be built, structurally validated, deterministically serialized for user review (the
//! Approval Gate), and content-addressed with a tamper-evident fingerprint.
//!
//! # Deliberate non-goals for this slice
//!
//! - No proof a source exists on disk. That is plan freshness (SI-006), enforced later by the
//!   planner/executor against a live filesystem.
//! - No real content hashing. [`ExpectedChecksum`] carries the *expected* digest a later verifier
//!   compares against. The plan fingerprint uses FNV-1a/128 purely for review-time tamper evidence
//!   and is replaced by BLAKE3 per ADR-004 when hashing lands.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

/// Schema tag emitted into every serialized plan. Bump on any breaking field change.
pub const ACTION_PLAN_SCHEMA_VERSION: &str = "action-plan-v1";

// ---------------------------------------------------------------------------
// UUIDv7 identifier
// ---------------------------------------------------------------------------

/// Injected millisecond Unix timestamp source for UUIDv7 construction.
///
/// Injecting the clock keeps identifiers deterministic under test and keeps the domain free of any
/// platform time dependency.
pub trait UuidV7Clock {
    /// Returns milliseconds since the Unix epoch. Only the low 48 bits are used.
    fn now_unix_millis(&self) -> u64;
}

/// Injected randomness source for the random bits of a UUIDv7.
///
/// Injecting the RNG keeps identifiers deterministic under test and keeps the domain free of any
/// platform randomness dependency.
pub trait UuidV7Rng {
    /// Returns the next 8 random bytes. Callers consume only the bits they need.
    fn next_random_bytes(&mut self) -> [u8; 8];
}

/// A time-ordered RFC 9562 UUID version 7 used as an action identifier.
///
/// Layout (network byte order): 48-bit Unix millis, 4-bit version `0b0111`, 12 random bits,
/// 2-bit variant `0b10`, 62 random bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId {
    bytes: [u8; 16],
}

impl ActionId {
    /// Builds a UUIDv7 from an injected clock and RNG.
    ///
    /// The timestamp is truncated to its low 48 bits. Version and variant bits are set
    /// unconditionally, so the result always satisfies [`ActionId::from_bytes`].
    pub fn new_v7(clock: &dyn UuidV7Clock, rng: &mut dyn UuidV7Rng) -> Self {
        let millis = clock.now_unix_millis() & 0x0000_FFFF_FFFF_FFFF;
        let random = rng.next_random_bytes();

        let mut bytes = [0u8; 16];
        bytes[0] = ((millis >> 40) & 0xFF) as u8;
        bytes[1] = ((millis >> 32) & 0xFF) as u8;
        bytes[2] = ((millis >> 24) & 0xFF) as u8;
        bytes[3] = ((millis >> 16) & 0xFF) as u8;
        bytes[4] = ((millis >> 8) & 0xFF) as u8;
        bytes[5] = (millis & 0xFF) as u8;

        // Version 7 in the high nibble of byte 6; 12 random bits across bytes 6..8.
        bytes[6] = 0x70 | (random[0] & 0x0F);
        bytes[7] = random[1];

        // Variant 0b10 in the high bits of byte 8; 62 random bits across bytes 8..16.
        bytes[8] = 0x80 | (random[2] & 0x3F);
        bytes[9] = random[3];
        bytes[10] = random[4];
        bytes[11] = random[5];
        bytes[12] = random[6];
        bytes[13] = random[7];
        bytes[14] = random[0] ^ random[4];
        bytes[15] = random[1] ^ random[5];

        Self { bytes }
    }

    /// Reconstructs an identifier from raw bytes, enforcing version and variant bits.
    ///
    /// This is the deserialization entry point. Malformed identifiers fail safely rather than
    /// round-tripping a value that is not a valid v7 UUID.
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, InvalidActionId> {
        if bytes[6] >> 4 != 0x7 {
            return Err(InvalidActionId::Version {
                observed: bytes[6] >> 4,
            });
        }
        if bytes[8] >> 6 != 0b10 {
            return Err(InvalidActionId::Variant {
                observed: bytes[8] >> 6,
            });
        }
        Ok(Self { bytes })
    }

    /// Parses the canonical 8-4-4-4-12 lowercase hyphenated hexadecimal form.
    pub fn parse_hyphenated(text: &str) -> Result<Self, InvalidActionId> {
        let bytes = text.as_bytes();
        if bytes.len() != 36 {
            return Err(InvalidActionId::Length {
                observed: bytes.len(),
            });
        }
        let mut raw = [0u8; 16];
        let mut raw_index = 0usize;
        let mut nibble_high: Option<u8> = None;
        for (position, byte) in bytes.iter().copied().enumerate() {
            if matches!(position, 8 | 13 | 18 | 23) {
                if byte != b'-' {
                    return Err(InvalidActionId::Separator { position });
                }
                continue;
            }
            let value = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => return Err(InvalidActionId::Digit { position }),
            };
            match nibble_high {
                None => nibble_high = Some(value),
                Some(high) => {
                    raw[raw_index] = (high << 4) | value;
                    raw_index += 1;
                    nibble_high = None;
                }
            }
        }
        Self::from_bytes(raw)
    }

    /// Returns the raw 16-byte big-endian representation.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    /// Returns the RFC 9562 version nibble, always `7` for a valid value.
    pub const fn version(&self) -> u8 {
        self.bytes[6] >> 4
    }

    /// Returns the 48-bit embedded Unix-millisecond timestamp.
    pub const fn unix_millis(&self) -> u64 {
        ((self.bytes[0] as u64) << 40)
            | ((self.bytes[1] as u64) << 32)
            | ((self.bytes[2] as u64) << 24)
            | ((self.bytes[3] as u64) << 16)
            | ((self.bytes[4] as u64) << 8)
            | (self.bytes[5] as u64)
    }

    /// Formats the identifier as canonical lowercase hyphenated hexadecimal.
    pub fn to_hyphenated(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(36);
        for (index, byte) in self.bytes.iter().copied().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                out.push('-');
            }
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
        out
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hyphenated())
    }
}

/// Reason an [`ActionId`] failed construction or parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidActionId {
    /// The hyphenated text was not exactly 36 characters.
    Length {
        /// Observed character length.
        observed: usize,
    },
    /// A hyphen was expected at this position but not found.
    Separator {
        /// Zero-based character position.
        position: usize,
    },
    /// A non-hexadecimal character was found at this position.
    Digit {
        /// Zero-based character position.
        position: usize,
    },
    /// The version nibble was not `7`.
    Version {
        /// Observed version nibble.
        observed: u8,
    },
    /// The variant bits were not `0b10`.
    Variant {
        /// Observed two-bit variant field.
        observed: u8,
    },
}

impl fmt::Display for InvalidActionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { observed } => {
                write!(formatter, "action id text must be 36 chars, got {observed}")
            }
            Self::Separator { position } => {
                write!(formatter, "expected '-' at position {position}")
            }
            Self::Digit { position } => {
                write!(formatter, "invalid hex digit at position {position}")
            }
            Self::Version { observed } => {
                write!(formatter, "action id version must be 7, got {observed}")
            }
            Self::Variant { observed } => {
                write!(formatter, "action id variant must be 0b10, got {observed:#b}")
            }
        }
    }
}

impl Error for InvalidActionId {}

// ---------------------------------------------------------------------------
// Plan path
// ---------------------------------------------------------------------------

/// A non-empty, display-preserving path string used inside a proposed plan.
///
/// The domain keeps a display representation and a normalized comparison representation without
/// reconstructing one from the other (ARCHITECTURE §9.2). Slice A comparison normalization is
/// deliberately minimal: it lowercases ASCII and unifies separators to `/` for structural checks
/// only. Real Windows identity/case semantics arrive with `fileos-platform-windows` (SI-002).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanPath {
    display: String,
    comparison: String,
}

impl PlanPath {
    /// Creates a plan path, rejecting empty input and raw NUL bytes.
    pub fn new(display: impl Into<String>) -> Result<Self, InvalidPlanPath> {
        let display = display.into();
        if display.is_empty() {
            return Err(InvalidPlanPath::Empty);
        }
        if display.contains('\0') {
            return Err(InvalidPlanPath::InteriorNul);
        }
        let comparison = display
            .chars()
            .map(|c| match c {
                '\\' => '/',
                other => other.to_ascii_lowercase(),
            })
            .collect();
        Ok(Self {
            display,
            comparison,
        })
    }

    /// Returns the original display path exactly as supplied.
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Returns the normalized comparison representation for structural checks.
    pub fn comparison(&self) -> &str {
        &self.comparison
    }

    /// Reports whether `self` is a strict descendant of `ancestor` by comparison representation.
    ///
    /// Uses a boundary-aware segment check so `.../database` is never treated as a child of
    /// `.../data` (ARCHITECTURE §7 sibling-confusion property).
    pub fn is_descendant_of(&self, ancestor: &PlanPath) -> bool {
        let child = self.comparison.trim_end_matches('/');
        let parent = ancestor.comparison.trim_end_matches('/');
        if child.len() <= parent.len() {
            return false;
        }
        match child.strip_prefix(parent) {
            Some(rest) => rest.starts_with('/'),
            None => false,
        }
    }
}

/// Reason a [`PlanPath`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPlanPath {
    /// The path was empty.
    Empty,
    /// The path contained an interior NUL byte.
    InteriorNul,
}

impl fmt::Display for InvalidPlanPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("plan path must not be empty"),
            Self::InteriorNul => formatter.write_str("plan path must not contain a NUL byte"),
        }
    }
}

impl Error for InvalidPlanPath {}

// ---------------------------------------------------------------------------
// Expected checksum
// ---------------------------------------------------------------------------

/// Content-hash algorithm recorded alongside an expected digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChecksumAlgorithm {
    /// BLAKE3, the project default (ADR-004). Digest is 32 bytes.
    Blake3,
    /// SHA-256, accepted for interoperability. Digest is 32 bytes.
    Sha256,
}

impl ChecksumAlgorithm {
    /// Returns the stable machine-readable code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
            Self::Sha256 => "sha256",
        }
    }

    /// Returns the required digest length in bytes.
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Blake3 | Self::Sha256 => 32,
        }
    }
}

/// The digest a later verifier must observe for content-certain actions (SI-006, SI-008).
///
/// This is the *expected* value carried by the plan. It is never computed from the filesystem in
/// this slice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpectedChecksum {
    algorithm: ChecksumAlgorithm,
    digest: Vec<u8>,
}

impl ExpectedChecksum {
    /// Creates an expected checksum, enforcing the algorithm's digest length.
    pub fn new(algorithm: ChecksumAlgorithm, digest: Vec<u8>) -> Result<Self, InvalidChecksum> {
        if digest.len() != algorithm.digest_len() {
            return Err(InvalidChecksum::Length {
                algorithm,
                expected: algorithm.digest_len(),
                observed: digest.len(),
            });
        }
        Ok(Self { algorithm, digest })
    }

    /// Returns the checksum algorithm.
    pub const fn algorithm(&self) -> ChecksumAlgorithm {
        self.algorithm
    }

    /// Returns the raw digest bytes.
    pub fn digest(&self) -> &[u8] {
        &self.digest
    }

    /// Returns the digest as lowercase hexadecimal.
    pub fn digest_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(self.digest.len() * 2);
        for byte in &self.digest {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
        out
    }
}

/// Reason an [`ExpectedChecksum`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidChecksum {
    /// The digest length did not match the algorithm.
    Length {
        /// The algorithm whose length was violated.
        algorithm: ChecksumAlgorithm,
        /// Required digest length in bytes.
        expected: usize,
        /// Observed digest length in bytes.
        observed: usize,
    },
}

impl fmt::Display for InvalidChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length {
                algorithm,
                expected,
                observed,
            } => write!(
                formatter,
                "{} digest must be {expected} bytes, got {observed}",
                algorithm.as_code()
            ),
        }
    }
}

impl Error for InvalidChecksum {}

// ---------------------------------------------------------------------------
// Action kind + planned action
// ---------------------------------------------------------------------------

/// The typed operation a planned action proposes. No variant performs I/O in this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Move a file to a new location, later executed atomically same-volume or copy->verify->retire
    /// cross-volume (ADR-006).
    Move,
    /// Rename a file within its current directory.
    Rename,
    /// Stage a file for deletion by moving it to Quarantine (ADR-005). Requires a checksum.
    StageForDeletion,
}

impl ActionKind {
    /// Returns the stable machine-readable code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Rename => "rename",
            Self::StageForDeletion => "stage_for_deletion",
        }
    }

    /// Reports whether this kind requires an expected checksum for content certainty.
    ///
    /// Destructive staging always requires one (SI-006/SI-008). Move and rename may carry one but do
    /// not require it at the domain layer; the executor enforces freshness against the filesystem.
    pub const fn requires_checksum(self) -> bool {
        matches!(self, Self::StageForDeletion)
    }
}

/// One immutable proposed action. Constructed only through [`ActionPlanBuilder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAction {
    id: ActionId,
    kind: ActionKind,
    source: PlanPath,
    destination: PlanPath,
    expected_checksum: Option<ExpectedChecksum>,
}

impl PlannedAction {
    /// Returns the time-ordered action identifier.
    pub const fn id(&self) -> &ActionId {
        &self.id
    }

    /// Returns the action kind.
    pub const fn kind(&self) -> ActionKind {
        self.kind
    }

    /// Returns the source path.
    pub const fn source(&self) -> &PlanPath {
        &self.source
    }

    /// Returns the destination path.
    pub const fn destination(&self) -> &PlanPath {
        &self.destination
    }

    /// Returns the expected post-conditions checksum, if any.
    pub const fn expected_checksum(&self) -> Option<&ExpectedChecksum> {
        self.expected_checksum.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Plan-level validation
// ---------------------------------------------------------------------------

/// A structural defect that makes a proposed plan unsafe to serialize or approve.
///
/// These are plan-internal invariants only (ARCHITECTURE §8.3, SAFETY §5 plan-level checks). Live
/// filesystem freshness (SI-006), scope (SI-002), and protected zones (SI-011) are enforced later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanValidationError {
    /// The plan contained no actions.
    Empty,
    /// A staging action was missing its mandatory checksum.
    MissingChecksum {
        /// The offending action id.
        action: ActionId,
    },
    /// Two actions targeted the same destination comparison path.
    DuplicateDestination {
        /// The colliding destination display path.
        destination: String,
    },
    /// Two actions declared the same source comparison path.
    DuplicateSource {
        /// The colliding source display path.
        source: String,
    },
    /// One action's source is another action's destination, an ordering hazard (SAFETY §5).
    SourceIsAnotherDestination {
        /// The overlapping path's display form.
        path: String,
    },
    /// A move/rename would place a directory-like source inside its own descendant.
    MoveIntoOwnDescendant {
        /// The offending action id.
        action: ActionId,
    },
    /// An action's source and destination comparison paths are identical (no-op / unsafe).
    SourceEqualsDestination {
        /// The offending action id.
        action: ActionId,
    },
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("an action plan must contain at least one action"),
            Self::MissingChecksum { action } => {
                write!(formatter, "action {action} requires an expected checksum")
            }
            Self::DuplicateDestination { destination } => {
                write!(formatter, "duplicate destination in plan: {destination}")
            }
            Self::DuplicateSource { source } => {
                write!(formatter, "duplicate source in plan: {source}")
            }
            Self::SourceIsAnotherDestination { path } => write!(
                formatter,
                "path is both a source and another action's destination: {path}"
            ),
            Self::MoveIntoOwnDescendant { action } => {
                write!(formatter, "action {action} moves a path into its own descendant")
            }
            Self::SourceEqualsDestination { action } => {
                write!(formatter, "action {action} has an identical source and destination")
            }
        }
    }
}

impl Error for PlanValidationError {}

// ---------------------------------------------------------------------------
// Plan fingerprint
// ---------------------------------------------------------------------------

/// A tamper-evident, content-addressed fingerprint of a serialized plan.
///
/// Slice A uses FNV-1a/128 over the canonical JSON bytes purely for review-time tamper evidence.
/// It is **not** a cryptographic hash and is replaced by BLAKE3 per ADR-004 when hashing lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanFingerprint {
    value: u128,
}

impl PlanFingerprint {
    const FNV_OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const FNV_PRIME: u128 = 0x0000000001000000000000000000013B;

    /// Computes the fingerprint over raw canonical bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hash = Self::FNV_OFFSET;
        for &byte in bytes {
            hash ^= byte as u128;
            hash = hash.wrapping_mul(Self::FNV_PRIME);
        }
        Self { value: hash }
    }

    /// Returns the raw 128-bit value.
    pub const fn value(self) -> u128 {
        self.value
    }

    /// Returns the fingerprint as fixed-width lowercase hexadecimal.
    pub fn to_hex(self) -> String {
        format!("{:032x}", self.value)
    }
}

impl fmt::Display for PlanFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

// ---------------------------------------------------------------------------
// Immutable action plan + builder
// ---------------------------------------------------------------------------

/// An immutable, validated, deterministically serializable proposed plan.
///
/// There is no public constructor and no mutator. The only way to obtain one is
/// [`ActionPlanBuilder::build`], which validates every plan-level invariant. Any edit means
/// building a new value, which yields a new [`PlanFingerprint`] (EP-007 acceptance criteria).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlan {
    actions: Vec<PlannedAction>,
}

impl ActionPlan {
    /// Returns the ordered actions. The slice is read-only; the plan cannot be mutated in place.
    pub fn actions(&self) -> &[PlannedAction] {
        &self.actions
    }

    /// Returns the number of actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Reports whether the plan has no actions. Always `false` for a built plan.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Serializes the plan to canonical `action-plan-v1` JSON.
    ///
    /// The writer is dependency-free and fully deterministic: field order is fixed, actions keep
    /// insertion order, no floats or maps are emitted, and strings are escaped per RFC 8259.
    /// Identical inputs therefore always yield byte-identical output (EP-007 acceptance criteria).
    pub fn to_canonical_json(&self) -> String {
        let mut out = String::new();
        out.push('{');
        push_json_field(&mut out, "schema", ACTION_PLAN_SCHEMA_VERSION, true);
        out.push_str("\"actions\":[");
        for (index, action) in self.actions.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push('{');
            push_json_field(&mut out, "id", &action.id().to_hyphenated(), true);
            push_json_field(&mut out, "kind", action.kind().as_code(), true);
            push_json_field(&mut out, "source", action.source().display(), true);
            push_json_field(&mut out, "source_cmp", action.source().comparison(), true);
            push_json_field(&mut out, "destination", action.destination().display(), true);
            push_json_field(
                &mut out,
                "destination_cmp",
                action.destination().comparison(),
                true,
            );
            match action.expected_checksum() {
                Some(checksum) => {
                    out.push_str("\"checksum\":{");
                    push_json_field(&mut out, "algorithm", checksum.algorithm().as_code(), true);
                    push_json_field(&mut out, "digest_hex", &checksum.digest_hex(), false);
                    out.push('}');
                }
                None => out.push_str("\"checksum\":null"),
            }
            out.push('}');
        }
        out.push(']');
        out.push('}');
        out
    }

    /// Computes the content-addressed fingerprint over the canonical JSON bytes.
    pub fn fingerprint(&self) -> PlanFingerprint {
        PlanFingerprint::of_bytes(self.to_canonical_json().as_bytes())
    }
}

/// Accumulates actions and enforces every plan-level invariant on [`ActionPlanBuilder::build`].
#[derive(Debug, Default)]
pub struct ActionPlanBuilder {
    actions: Vec<PlannedAction>,
}

impl ActionPlanBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Appends one action, generating a fresh UUIDv7 from the injected clock and RNG.
    ///
    /// Per-action invariants are checked here (checksum requirement, source != destination). Cross-
    /// action invariants are checked in [`ActionPlanBuilder::build`].
    pub fn push_action(
        &mut self,
        kind: ActionKind,
        source: PlanPath,
        destination: PlanPath,
        expected_checksum: Option<ExpectedChecksum>,
        clock: &dyn UuidV7Clock,
        rng: &mut dyn UuidV7Rng,
    ) -> Result<&mut Self, PlanValidationError> {
        let id = ActionId::new_v7(clock, rng);
        if kind.requires_checksum() && expected_checksum.is_none() {
            return Err(PlanValidationError::MissingChecksum { action: id });
        }
        if source.comparison() == destination.comparison() {
            return Err(PlanValidationError::SourceEqualsDestination { action: id });
        }
        self.actions.push(PlannedAction {
            id,
            kind,
            source,
            destination,
            expected_checksum,
        });
        Ok(self)
    }

    /// Validates every plan-level invariant and returns an immutable [`ActionPlan`].
    ///
    /// Rejections: empty plan, duplicate destination, duplicate source, a source that is another
    /// action's destination, and a move/rename into the source's own descendant.
    pub fn build(self) -> Result<ActionPlan, PlanValidationError> {
        if self.actions.is_empty() {
            return Err(PlanValidationError::Empty);
        }

        let mut sources: BTreeSet<&str> = BTreeSet::new();
        let mut destinations: BTreeSet<&str> = BTreeSet::new();

        for action in &self.actions {
            if !destinations.insert(action.destination().comparison()) {
                return Err(PlanValidationError::DuplicateDestination {
                    destination: action.destination().display().to_string(),
                });
            }
            if !sources.insert(action.source().comparison()) {
                return Err(PlanValidationError::DuplicateSource {
                    source: action.source().display().to_string(),
                });
            }
            if action.destination().is_descendant_of(action.source()) {
                return Err(PlanValidationError::MoveIntoOwnDescendant { action: *action.id() });
            }
        }

        for action in &self.actions {
            if destinations.contains(action.source().comparison()) {
                return Err(PlanValidationError::SourceIsAnotherDestination {
                    path: action.source().display().to_string(),
                });
            }
        }

        Ok(ActionPlan {
            actions: self.actions,
        })
    }
}

/// Appends a `"key":"value"` pair with RFC 8259 string escaping, optionally followed by a comma.
fn push_json_field(out: &mut String, key: &str, value: &str, trailing_comma: bool) {
    out.push('"');
    push_json_escaped(out, key);
    out.push_str("\":\"");
    push_json_escaped(out, value);
    out.push('"');
    if trailing_comma {
        out.push(',');
    }
}

/// Escapes a string per RFC 8259 into `out`.
fn push_json_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Deterministic fixed clock for reproducible identifiers.
    struct FixedClock(u64);
    impl UuidV7Clock for FixedClock {
        fn now_unix_millis(&self) -> u64 {
            self.0
        }
    }

    /// Deterministic counter-based RNG for reproducible identifiers.
    struct CountingRng(u64);
    impl UuidV7Rng for CountingRng {
        fn next_random_bytes(&mut self) -> [u8; 8] {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            self.0.to_be_bytes()
        }
    }

    fn path(value: &str) -> PlanPath {
        PlanPath::new(value).expect("valid test path")
    }

    fn sample_checksum() -> ExpectedChecksum {
        ExpectedChecksum::new(ChecksumAlgorithm::Blake3, vec![0xABu8; 32])
            .expect("32-byte blake3 digest")
    }

    #[test]
    fn uuid_v7_has_correct_version_and_variant() {
        let clock = FixedClock(0x0000_0189_5C2E_1F00);
        let mut rng = CountingRng(1);
        let id = ActionId::new_v7(&clock, &mut rng);
        assert_eq!(id.version(), 7);
        assert_eq!(id.as_bytes()[8] >> 6, 0b10);
        assert_eq!(id.unix_millis(), 0x0000_0189_5C2E_1F00 & 0x0000_FFFF_FFFF_FFFF);
    }

    #[test]
    fn uuid_v7_round_trips_hyphenated_form() {
        let clock = FixedClock(42);
        let mut rng = CountingRng(7);
        let id = ActionId::new_v7(&clock, &mut rng);
        let text = id.to_hyphenated();
        assert_eq!(text.len(), 36);
        let parsed = ActionId::parse_hyphenated(&text).expect("round-trip parse");
        assert_eq!(parsed, id);
    }

    #[test]
    fn uuid_v7_rejects_wrong_version_and_variant() {
        let mut bytes = [0u8; 16];
        bytes[8] = 0x80; // valid variant, invalid version
        assert!(matches!(
            ActionId::from_bytes(bytes),
            Err(InvalidActionId::Version { .. })
        ));
        let mut bytes = [0u8; 16];
        bytes[6] = 0x70; // valid version, invalid variant
        assert!(matches!(
            ActionId::from_bytes(bytes),
            Err(InvalidActionId::Variant { .. })
        ));
    }

    #[test]
    fn plan_path_rejects_empty_and_nul() {
        assert_eq!(PlanPath::new(""), Err(InvalidPlanPath::Empty));
        assert_eq!(PlanPath::new("a\0b"), Err(InvalidPlanPath::InteriorNul));
    }

    #[test]
    fn descendant_check_is_boundary_aware() {
        let data = path("C:/data");
        let database = path("C:/database/file.txt");
        let child = path("C:/data/sub/file.txt");
        assert!(!database.is_descendant_of(&data));
        assert!(child.is_descendant_of(&data));
    }

    #[test]
    fn checksum_enforces_digest_length() {
        assert!(matches!(
            ExpectedChecksum::new(ChecksumAlgorithm::Blake3, vec![0u8; 31]),
            Err(InvalidChecksum::Length { .. })
        ));
        assert!(ExpectedChecksum::new(ChecksumAlgorithm::Sha256, vec![0u8; 32]).is_ok());
    }

    #[test]
    fn staging_without_checksum_is_rejected() {
        let clock = FixedClock(1);
        let mut rng = CountingRng(1);
        let mut builder = ActionPlanBuilder::new();
        let result = builder.push_action(
            ActionKind::StageForDeletion,
            path("C:/in/dup.bin"),
            path("C:/.mh-fileos/quarantine/op/act/dup.bin"),
            None,
            &clock,
            &mut rng,
        );
        assert!(matches!(
            result,
            Err(PlanValidationError::MissingChecksum { .. })
        ));
    }

    #[test]
    fn empty_plan_is_rejected() {
        assert_eq!(ActionPlanBuilder::new().build(), Err(PlanValidationError::Empty));
    }

    #[test]
    fn duplicate_destination_is_rejected() {
        let clock = FixedClock(1);
        let mut rng = CountingRng(1);
        let mut builder = ActionPlanBuilder::new();
        builder
            .push_action(ActionKind::Move, path("C:/a.txt"), path("C:/out/x.txt"), None, &clock, &mut rng)
            .expect("first action");
        builder
            .push_action(ActionKind::Move, path("C:/b.txt"), path("C:/out/x.txt"), None, &clock, &mut rng)
            .expect("second action pushes");
        assert!(matches!(
            builder.build(),
            Err(PlanValidationError::DuplicateDestination { .. })
        ));
    }

    #[test]
    fn source_that_is_another_destination_is_rejected() {
        let clock = FixedClock(1);
        let mut rng = CountingRng(1);
        let mut builder = ActionPlanBuilder::new();
        builder
            .push_action(ActionKind::Move, path("C:/a.txt"), path("C:/b.txt"), None, &clock, &mut rng)
            .expect("first action");
        builder
            .push_action(ActionKind::Move, path("C:/b.txt"), path("C:/c.txt"), None, &clock, &mut rng)
            .expect("second action");
        assert!(matches!(
            builder.build(),
            Err(PlanValidationError::SourceIsAnotherDestination { .. })
        ));
    }

    #[test]
    fn move_into_own_descendant_is_rejected() {
        let clock = FixedClock(1);
        let mut rng = CountingRng(1);
        let mut builder = ActionPlanBuilder::new();
        builder
            .push_action(
                ActionKind::Move,
                path("C:/dir"),
                path("C:/dir/child"),
                None,
                &clock,
                &mut rng,
            )
            .expect("push");
        assert!(matches!(
            builder.build(),
            Err(PlanValidationError::MoveIntoOwnDescendant { .. })
        ));
    }

    #[test]
    fn valid_plan_serializes_deterministically_and_is_content_addressed() {
        let build_plan = || {
            let clock = FixedClock(0x0189_5C2E_1F00);
            let mut rng = CountingRng(99);
            let mut builder = ActionPlanBuilder::new();
            builder
                .push_action(
                    ActionKind::StageForDeletion,
                    path("C:/in/dup.bin"),
                    path("C:/.mh-fileos/quarantine/op/act/dup.bin"),
                    Some(sample_checksum()),
                    &clock,
                    &mut rng,
                )
                .expect("staging action");
            builder
                .push_action(
                    ActionKind::Move,
                    path("C:/in/report.pdf"),
                    path("C:/sorted/report.pdf"),
                    None,
                    &clock,
                    &mut rng,
                )
                .expect("move action");
            builder.build().expect("valid plan")
        };

        let first = build_plan();
        let second = build_plan();
        assert_eq!(first.to_canonical_json(), second.to_canonical_json());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert!(first.to_canonical_json().contains("\"schema\":\"action-plan-v1\""));
        assert_eq!(first.len(), 2);
    }

    proptest! {
        // Identical inputs always yield byte-identical canonical JSON and equal fingerprints.
        #[test]
        fn prop_serialization_is_deterministic(seed in any::<u64>(), millis in any::<u64>()) {
            let build_plan = || {
                let clock = FixedClock(millis);
                let mut rng = CountingRng(seed);
                let mut builder = ActionPlanBuilder::new();
                builder
                    .push_action(
                        ActionKind::Move,
                        path("C:/in/a.txt"),
                        path("C:/out/a.txt"),
                        None,
                        &clock,
                        &mut rng,
                    )
                    .expect("push");
                builder.build().expect("plan")
            };
            let a = build_plan();
            let b = build_plan();
            prop_assert_eq!(a.to_canonical_json(), b.to_canonical_json());
            prop_assert_eq!(a.fingerprint(), b.fingerprint());
        }

        // Any generated v7 id round-trips through its hyphenated form.
        #[test]
        fn prop_action_id_round_trips(seed in any::<u64>(), millis in any::<u64>()) {
            let clock = FixedClock(millis);
            let mut rng = CountingRng(seed);
            let id = ActionId::new_v7(&clock, &mut rng);
            let parsed = ActionId::parse_hyphenated(&id.to_hyphenated()).expect("round trip");
            prop_assert_eq!(parsed, id);
            prop_assert_eq!(id.version(), 7);
        }

        // A distinct destination never trips the duplicate-destination check.
        #[test]
        fn prop_distinct_destinations_build(count in 1usize..12) {
            let clock = FixedClock(1);
            let mut rng = CountingRng(1);
            let mut builder = ActionPlanBuilder::new();
            for index in 0..count {
                builder
                    .push_action(
                        ActionKind::Move,
                        PlanPath::new(format!("C:/in/src-{index}.txt")).expect("src"),
                        PlanPath::new(format!("C:/out/dst-{index}.txt")).expect("dst"),
                        None,
                        &clock,
                        &mut rng,
                    )
                    .expect("push");
            }
            let plan = builder.build().expect("distinct plan builds");
            prop_assert_eq!(plan.len(), count);
        }
    }
}
