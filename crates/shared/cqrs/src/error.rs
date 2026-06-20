use std::fmt;

use http::StatusCode;

use ::error::{AppError, Severity};

/// Type-erased container that preserves all [`AppError`] metadata from a
/// concrete handler error while remaining object-safe.
///
/// Constructed once via [`BoxedDynAppError::new`] at the registry boundary;
/// never constructed by external callers.
pub struct BoxedDynAppError {
    error_code: &'static str,
    http_status: StatusCode,
    severity: Severity,
    is_retryable: bool,
    category: &'static str,
    user_facing_message: &'static str,
    /// Original error kept for Display delegation and error-source chaining.
    inner: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl BoxedDynAppError {
    pub(crate) fn new<E: AppError>(e: E) -> Self {
        Self {
            error_code: e.error_code(),
            http_status: e.http_status(),
            severity: e.severity(),
            is_retryable: e.is_retryable(),
            category: e.category(),
            user_facing_message: e.user_facing_message(),
            inner: Box::new(e),
        }
    }
}

impl fmt::Debug for BoxedDynAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxedDynAppError")
            .field("error_code", &self.error_code)
            .field("severity", &self.severity)
            .field("inner", &self.inner)
            .finish()
    }
}

impl fmt::Display for BoxedDynAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.inner, f)
    }
}

impl std::error::Error for BoxedDynAppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl AppError for BoxedDynAppError {
    fn error_code(&self) -> &'static str { self.error_code }
    fn http_status(&self) -> StatusCode { self.http_status }
    fn severity(&self) -> Severity { self.severity }
    fn is_retryable(&self) -> bool { self.is_retryable }
    fn category(&self) -> &'static str { self.category }
    fn user_facing_message(&self) -> &'static str { self.user_facing_message }
}

/// Bus-level error returned by every [`CommandBus::dispatch`] and
/// [`QueryBus::dispatch`] call.
///
/// Distinguishes between infrastructure failures (`HandlerNotFound`,
/// `DuplicateRegistration`) and domain failures (`Handler`). The `Handler`
/// variant transparently delegates all [`AppError`] methods to the original
/// error so callers can treat `CqrsError` uniformly.
#[derive(Debug)]
pub enum CqrsError {
    /// No handler was registered for the given command or query type.
    /// This is a programming error and should never happen in production.
    HandlerNotFound { type_name: &'static str },

    /// A handler for the given type was registered more than once on the
    /// same bus builder. Detected at construction time, not at dispatch time.
    DuplicateRegistration { type_name: &'static str },

    /// The registered handler returned an error. All [`AppError`] metadata
    /// is preserved and accessible via the [`AppError`] impl on `CqrsError`.
    Handler(BoxedDynAppError),
}

impl CqrsError {
    /// Wraps any [`AppError`] implementation into `CqrsError::Handler`.
    ///
    /// Called exclusively by the type-erased registry bridge so that handler
    /// errors are promoted to the bus error type without losing metadata.
    pub(crate) fn from_handler<E: AppError>(e: E) -> Self {
        Self::Handler(BoxedDynAppError::new(e))
    }
}

impl fmt::Display for CqrsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandlerNotFound { type_name } => {
                write!(f, "no handler registered for `{type_name}`")
            }
            Self::DuplicateRegistration { type_name } => {
                write!(f, "handler already registered for `{type_name}`")
            }
            Self::Handler(e) => write!(f, "handler execution failed: {e}"),
        }
    }
}

impl std::error::Error for CqrsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Handler(e) => Some(e),
            _ => None,
        }
    }
}

impl AppError for CqrsError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::HandlerNotFound { .. } => "CQRS_HANDLER_NOT_FOUND",
            Self::DuplicateRegistration { .. } => "CQRS_DUPLICATE_REGISTRATION",
            Self::Handler(e) => e.error_code(),
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            Self::HandlerNotFound { .. } | Self::DuplicateRegistration { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Handler(e) => e.http_status(),
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::HandlerNotFound { .. } | Self::DuplicateRegistration { .. } => Severity::Critical,
            Self::Handler(e) => e.severity(),
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::HandlerNotFound { .. } | Self::DuplicateRegistration { .. } => false,
            Self::Handler(e) => e.is_retryable(),
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::HandlerNotFound { .. } | Self::DuplicateRegistration { .. } => "CQRS",
            Self::Handler(e) => e.category(),
        }
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            Self::HandlerNotFound { .. } | Self::DuplicateRegistration { .. } => {
                "An internal error occurred."
            }
            Self::Handler(e) => e.user_facing_message(),
        }
    }
}
