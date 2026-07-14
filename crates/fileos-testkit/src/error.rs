use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Result type used by fixture creation and validation operations.
pub type Result<T> = std::result::Result<T, TestkitError>;

/// Typed failure returned by the isolated fixture testkit.
#[derive(Debug)]
pub enum TestkitError {
    /// A filesystem operation failed inside an already-scoped testkit path.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// The provided directory is not positively identified as this repository.
    InvalidRepository { reason: &'static str },
    /// A path failed the approved sandbox containment contract.
    ScopeViolation { reason: &'static str },
    /// A fixture-relative path was absolute or contained traversal/prefix components.
    InvalidRelativePath { path: PathBuf },
    /// The marker or manifest no longer matches the capability that created the run.
    IdentityMismatch { artifact: &'static str },
    /// The system clock could not produce a usable unique run identity.
    ClockUnavailable,
    /// Bounded attempts could not allocate a unique run directory.
    RunAllocationExhausted,
    /// A read-only fixture snapshot exceeded its explicit evidence bound.
    SnapshotLimitExceeded { resource: &'static str, limit: u64 },
    /// Fixture population failed and the follow-up cleanup also failed closed.
    CreationCleanupFailed {
        creation: Box<TestkitError>,
        cleanup: Box<TestkitError>,
    },
}

impl TestkitError {
    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for TestkitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "testkit operation '{operation}' failed for '{}': {source}",
                path.display()
            ),
            Self::InvalidRepository { reason } => {
                write!(formatter, "repository identity check failed: {reason}")
            }
            Self::ScopeViolation { reason } => {
                write!(formatter, "sandbox scope validation failed: {reason}")
            }
            Self::InvalidRelativePath { path } => write!(
                formatter,
                "fixture path must be a traversal-free relative path: '{}'",
                path.display()
            ),
            Self::IdentityMismatch { artifact } => {
                write!(formatter, "sandbox identity mismatch in {artifact}")
            }
            Self::ClockUnavailable => {
                formatter.write_str("system clock cannot provide a sandbox run identity")
            }
            Self::RunAllocationExhausted => {
                formatter.write_str("bounded sandbox run allocation attempts were exhausted")
            }
            Self::SnapshotLimitExceeded { resource, limit } => write!(
                formatter,
                "fixture snapshot {resource} exceeded configured limit {limit}"
            ),
            Self::CreationCleanupFailed { creation, cleanup } => write!(
                formatter,
                "fixture creation failed ({creation}); fail-closed cleanup also failed ({cleanup})"
            ),
        }
    }
}

impl Error for TestkitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::CreationCleanupFailed { creation, .. } => Some(creation.as_ref()),
            _ => None,
        }
    }
}

/// A failed cleanup retains both the error and the unconsumed run capability.
///
/// This lets callers inspect or repair a deliberately tampered test fixture without broadening
/// cleanup scope or reconstructing a capability from a raw path.
#[derive(Debug)]
pub struct CleanupFailure {
    pub(crate) error: TestkitError,
    pub(crate) run: crate::SandboxRun,
}

impl CleanupFailure {
    /// Returns the typed reason cleanup stopped.
    pub fn error(&self) -> &TestkitError {
        &self.error
    }

    /// Recovers the run capability and error for an explicit retry or inspection workflow.
    pub fn into_parts(self) -> (crate::SandboxRun, TestkitError) {
        (self.run, self.error)
    }
}

impl fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
