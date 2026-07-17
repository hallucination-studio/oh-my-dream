use std::fmt;

use thiserror::Error;

const MAX_CREDENTIAL_ID_LENGTH: usize = 128;
const MAX_CREDENTIAL_SECRET_LENGTH: usize = 16 * 1024;

macro_rules! credential_id {
    ($name:ident, $error:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Creates a validated lowercase dot-segment credential identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, $error> {
                let value = value.into();
                if is_valid_credential_id(&value) {
                    Ok(Self(value))
                } else {
                    Err($error::InvalidCredential)
                }
            }

            /// Returns the stable credential identifier.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! credential_secret {
    ($name:ident, $error:ident) => {
        pub struct $name(Vec<u8>);

        impl $name {
            /// Wraps non-empty credential bytes without exposing them in diagnostics.
            pub fn new(value: Vec<u8>) -> Result<Self, $error> {
                if value.is_empty() || value.len() > MAX_CREDENTIAL_SECRET_LENGTH {
                    Err($error::InvalidCredential)
                } else {
                    Ok(Self(value))
                }
            }

            /// Borrows the secret for immediate adapter use.
            pub fn as_bytes(&self) -> &[u8] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                self.0.fill(0);
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationProviderCredentialRepositoryError {
    #[error("invalid generation-provider credential")]
    InvalidCredential,
    #[error("generation-provider credential not found")]
    NotFound,
    #[error("generation-provider credential repository permission denied")]
    PermissionDenied,
    #[error("generation-provider credential repository unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AssistantModelCredentialRepositoryError {
    #[error("invalid Assistant-model credential")]
    InvalidCredential,
    #[error("Assistant-model credential not found")]
    NotFound,
    #[error("Assistant-model credential repository permission denied")]
    PermissionDenied,
    #[error("Assistant-model credential repository unavailable")]
    Unavailable,
}

credential_id!(GenerationProviderCredentialId, GenerationProviderCredentialRepositoryError);
credential_id!(AssistantModelCredentialId, AssistantModelCredentialRepositoryError);
credential_secret!(GenerationProviderCredentialSecret, GenerationProviderCredentialRepositoryError);
credential_secret!(AssistantModelCredentialSecret, AssistantModelCredentialRepositoryError);

fn is_valid_credential_id(value: &str) -> bool {
    if value.len() < 3 || value.len() > MAX_CREDENTIAL_ID_LENGTH {
        return false;
    }
    let mut segments = value.split('.');
    let valid_segment = |segment: &str| {
        let mut characters = segment.chars();
        characters.next().is_some_and(|character| character.is_ascii_lowercase())
            && characters.all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            })
    };
    let first = segments.next().is_some_and(valid_segment);
    let remaining: Vec<_> = segments.collect();
    first && !remaining.is_empty() && remaining.into_iter().all(valid_segment)
}
