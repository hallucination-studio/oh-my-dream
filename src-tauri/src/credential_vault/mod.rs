//! Separate generation-provider and Assistant-model credential boundaries.

mod interface;
mod value;

pub use interface::{
    AssistantModelCredentialVaultInterface, GenerationProviderCredentialVaultInterface,
};
pub use value::{
    AssistantModelCredentialId, AssistantModelCredentialSecret, AssistantModelCredentialVaultError,
    GenerationProviderCredentialId, GenerationProviderCredentialSecret,
    GenerationProviderCredentialVaultError,
};
