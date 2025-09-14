use std::fmt;

// Simplified error type - only variants that affect exit codes
#[derive(Debug)]
pub enum TerraformError {
    NonMonotonicTimeline,
    UserCancelled,
    NoMatchingCommits,
}

impl fmt::Display for TerraformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonMonotonicTimeline => write!(f, "Non-monotonic timeline detected"),
            Self::UserCancelled => write!(f, "Operation cancelled by user"),
            Self::NoMatchingCommits => write!(f, "No commits match specified criteria"),
        }
    }
}

impl std::error::Error for TerraformError {}

// Get exit code from any error type
pub fn exit_code(error: &anyhow::Error) -> i32 {
    // Check if it's one of our specific errors with custom exit codes
    if error.downcast_ref::<TerraformError>().is_some() {
        match error.downcast_ref::<TerraformError>().unwrap() {
            TerraformError::NonMonotonicTimeline => 2,
            TerraformError::UserCancelled => 3,
            TerraformError::NoMatchingCommits => 4,
        }
    } else {
        // All other errors (git, parse, invalid range, etc.) use exit code 1
        1
    }
}