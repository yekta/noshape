//! Error type of the kernel.

/// Errors returned by kernel operations.
///
/// Every OCCT exception (including `Standard_Failure` subclasses) thrown during
/// an operation is caught at the C++ boundary and converted into one of these
/// variants; the kernel never panics or aborts on modeling failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KernelError {
    /// The input was rejected before running the algorithm (bad parameter,
    /// wrong shape type, sub-shape not part of the shape, ...).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The modeling algorithm ran but failed to produce a result
    /// (e.g. fillet radius too large, boolean failure).
    #[error("operation failed: {0}")]
    OperationFailed(String),
    /// OCCT raised an exception (type and message are included).
    #[error("geometry kernel exception: {0}")]
    Occt(String),
    /// The requested operation is not supported by this kernel.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// File / serialization error.
    #[error("i/o error: {0}")]
    Io(String),
}

/// Kernel result type.
pub type Result<T, E = KernelError> = std::result::Result<T, E>;

impl KernelError {
    pub(crate) fn from_message(msg: &str) -> Self {
        if let Some(m) = msg.strip_prefix("invalid: ") {
            KernelError::InvalidInput(m.to_owned())
        } else if let Some(m) = msg.strip_prefix("failed: ") {
            KernelError::OperationFailed(m.to_owned())
        } else if let Some(m) = msg.strip_prefix("unsupported: ") {
            KernelError::Unsupported(m.to_owned())
        } else if let Some(m) = msg.strip_prefix("io: ") {
            KernelError::Io(m.to_owned())
        } else if let Some(m) = msg.strip_prefix("occt: ") {
            KernelError::Occt(m.to_owned())
        } else {
            KernelError::Occt(msg.to_owned())
        }
    }

    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        KernelError::InvalidInput(msg.into())
    }
}

impl From<cxx::Exception> for KernelError {
    fn from(e: cxx::Exception) -> Self {
        KernelError::from_message(e.what())
    }
}

impl From<std::io::Error> for KernelError {
    fn from(e: std::io::Error) -> Self {
        KernelError::Io(e.to_string())
    }
}
