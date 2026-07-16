//! Separate plaintext generation-provider and Assistant-model credential repositories.

mod interface;
mod value;

pub use interface::{
    AssistantModelCredentialRepositoryInterface, GenerationProviderCredentialRepositoryInterface,
};
pub use value::{
    AssistantModelCredentialId, AssistantModelCredentialRepositoryError,
    AssistantModelCredentialSecret, GenerationProviderCredentialId,
    GenerationProviderCredentialRepositoryError, GenerationProviderCredentialSecret,
};
