use std::fmt;

const MAX_BASE_URL_BYTES: usize = 2_048;
const MAX_MODEL_ID_BYTES: usize = 256;
const MAX_API_KEY_BYTES: usize = 16 * 1024;
const MAX_MODEL_COUNT: usize = 10_000;

/// Invalid Assistant Provider Settings value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AssistantProviderSettingsValueError {
    /// The Base URL is not a supported normalized HTTP(S) endpoint.
    #[error("Assistant provider Base URL is invalid")]
    InvalidBaseUrl,
    /// The provider-native model identifier is invalid.
    #[error("Assistant provider model ID is invalid")]
    InvalidModelId,
    /// The write-only API key is empty or oversized.
    #[error("Assistant provider API key is invalid")]
    InvalidApiKey,
    /// Persisted settings violate the enabled-state invariant.
    #[error("Assistant provider settings snapshot is invalid")]
    InvalidSnapshot,
}

/// Normalized user-selected HTTP(S) root for the Assistant provider.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssistantProviderBaseUrl(String);

impl AssistantProviderBaseUrl {
    /// Parses, validates, and removes trailing path slashes.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, AssistantProviderSettingsValueError> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.len() > MAX_BASE_URL_BYTES {
            return Err(AssistantProviderSettingsValueError::InvalidBaseUrl);
        }
        let parsed = url::Url::parse(value)
            .map_err(|_| AssistantProviderSettingsValueError::InvalidBaseUrl)?;
        let valid = matches!(parsed.scheme(), "http" | "https")
            && !parsed.cannot_be_a_base()
            && parsed.host().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none();
        if !valid {
            return Err(AssistantProviderSettingsValueError::InvalidBaseUrl);
        }
        let normalized = parsed.as_str().trim_end_matches('/').to_owned();
        if normalized.is_empty() || normalized.len() > MAX_BASE_URL_BYTES {
            Err(AssistantProviderSettingsValueError::InvalidBaseUrl)
        } else {
            Ok(Self(normalized))
        }
    }

    /// Returns the canonical URL text without trailing slashes.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated provider-native model identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssistantProviderModelId(String);

impl AssistantProviderModelId {
    /// Trims and validates one model identifier.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, AssistantProviderSettingsValueError> {
        let value = value.as_ref().trim();
        if value.is_empty()
            || value.len() > MAX_MODEL_ID_BYTES
            || value.chars().any(char::is_control)
        {
            Err(AssistantProviderSettingsValueError::InvalidModelId)
        } else {
            Ok(Self(value.to_owned()))
        }
    }

    /// Returns the normalized model identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-zero monotonic Assistant Provider Settings revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssistantProviderSettingsRevision(u64);

impl AssistantProviderSettingsRevision {
    /// Creates a non-zero Settings revision.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the revision integer.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Bounded write-only API key with redacted diagnostics.
#[derive(Eq, PartialEq)]
pub struct AssistantProviderApiKey(Vec<u8>);

impl AssistantProviderApiKey {
    /// Wraps non-empty API-key bytes.
    pub fn try_new(value: Vec<u8>) -> Result<Self, AssistantProviderSettingsValueError> {
        if value.is_empty() || value.len() > MAX_API_KEY_BYTES {
            Err(AssistantProviderSettingsValueError::InvalidApiKey)
        } else {
            Ok(Self(value))
        }
    }

    /// Borrows the secret for immediate boundary use.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for AssistantProviderApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AssistantProviderApiKey([REDACTED])")
    }
}

impl Drop for AssistantProviderApiKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Persisted sanitized Assistant connection state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantProviderSettingsSnapshot {
    revision: AssistantProviderSettingsRevision,
    enabled: bool,
    base_url: AssistantProviderBaseUrl,
    model_id: Option<AssistantProviderModelId>,
    has_api_key: bool,
}

impl AssistantProviderSettingsSnapshot {
    /// Restores a snapshot while enforcing the enabled-state invariant.
    pub fn try_new(
        revision: AssistantProviderSettingsRevision,
        enabled: bool,
        base_url: AssistantProviderBaseUrl,
        model_id: Option<AssistantProviderModelId>,
        has_api_key: bool,
    ) -> Result<Self, AssistantProviderSettingsValueError> {
        if enabled && (model_id.is_none() || !has_api_key) {
            return Err(AssistantProviderSettingsValueError::InvalidSnapshot);
        }
        Ok(Self { revision, enabled, base_url, model_id, has_api_key })
    }

    /// Returns the current settings revision.
    #[must_use]
    pub const fn revision(&self) -> AssistantProviderSettingsRevision {
        self.revision
    }

    /// Returns whether new Assistant invocations are enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the normalized provider Base URL.
    #[must_use]
    pub const fn base_url(&self) -> &AssistantProviderBaseUrl {
        &self.base_url
    }

    /// Returns the selected model, when one has been tested.
    #[must_use]
    pub const fn model_id(&self) -> Option<&AssistantProviderModelId> {
        self.model_id.as_ref()
    }

    /// Returns whether the fixed Assistant credential exists.
    #[must_use]
    pub const fn has_api_key(&self) -> bool {
        self.has_api_key
    }
}

/// Complete sanitized Assistant Provider Settings read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantProviderSettingsView {
    /// Current persisted Settings revision.
    pub settings_revision: AssistantProviderSettingsRevision,
    /// Whether new Assistant invocations are enabled.
    pub enabled: bool,
    /// Current normalized Base URL.
    pub base_url: AssistantProviderBaseUrl,
    /// Last successfully tested model, when present.
    pub model_id: Option<AssistantProviderModelId>,
    /// Whether the fixed write-only credential exists.
    pub has_api_key: bool,
}

impl From<AssistantProviderSettingsSnapshot> for AssistantProviderSettingsView {
    fn from(snapshot: AssistantProviderSettingsSnapshot) -> Self {
        Self {
            settings_revision: snapshot.revision,
            enabled: snapshot.enabled,
            base_url: snapshot.base_url,
            model_id: snapshot.model_id,
            has_api_key: snapshot.has_api_key,
        }
    }
}

/// One validated atomic Assistant Provider Settings mutation.
#[derive(Debug, Eq, PartialEq)]
pub enum AssistantProviderSettingsMutation {
    /// Enables one connection only after its compatibility test succeeded.
    ApplyTestedConnection {
        /// Candidate normalized Base URL.
        base_url: AssistantProviderBaseUrl,
        /// Candidate tested model identifier.
        model_id: AssistantProviderModelId,
        /// Replacement key, or `None` to retain the stored key.
        api_key: Option<AssistantProviderApiKey>,
    },
    /// Disables new invocations while retaining the tested connection.
    Disable,
}

/// Atomic Settings repository mutation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssistantProviderSettingsMutationResult {
    /// State changed and the revision advanced once.
    Committed(AssistantProviderSettingsSnapshot),
    /// The expected Settings revision was stale and nothing changed.
    RevisionConflict,
}

pub(crate) fn normalize_model_list(
    mut models: Vec<AssistantProviderModelId>,
) -> Result<Vec<AssistantProviderModelId>, AssistantProviderSettingsValueError> {
    models.sort();
    models.dedup();
    if models.len() > MAX_MODEL_COUNT {
        Err(AssistantProviderSettingsValueError::InvalidSnapshot)
    } else {
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_base_urls_and_normalizes_trailing_slashes() {
        let http = AssistantProviderBaseUrl::try_new("http://127.0.0.1:11434/v1///").unwrap();
        let https = AssistantProviderBaseUrl::try_new("https://api.openai.com/v1/").unwrap();

        assert_eq!(http.as_str(), "http://127.0.0.1:11434/v1");
        assert_eq!(https.as_str(), "https://api.openai.com/v1");
    }

    #[test]
    fn rejects_unsupported_or_unsafe_base_url_components() {
        for value in [
            "ftp://example.com/v1",
            "https://user@example.com/v1",
            "https://example.com/v1?key=value",
            "https://example.com/v1#models",
            "not-a-url",
        ] {
            assert_eq!(
                AssistantProviderBaseUrl::try_new(value),
                Err(AssistantProviderSettingsValueError::InvalidBaseUrl),
                "unexpectedly accepted {value}"
            );
        }
    }

    #[test]
    fn rejects_oversized_base_url() {
        let value = format!("https://example.com/{}", "a".repeat(2_048));

        assert_eq!(
            AssistantProviderBaseUrl::try_new(value),
            Err(AssistantProviderSettingsValueError::InvalidBaseUrl)
        );
    }

    #[test]
    fn trims_model_ids_and_rejects_empty_control_or_oversized_values() {
        let model = AssistantProviderModelId::try_new("  gpt-5.4  ").unwrap();
        assert_eq!(model.as_str(), "gpt-5.4");

        for value in [String::new(), "bad\nmodel".to_owned(), "m".repeat(257)] {
            assert_eq!(
                AssistantProviderModelId::try_new(value),
                Err(AssistantProviderSettingsValueError::InvalidModelId)
            );
        }
    }

    #[test]
    fn revision_is_non_zero() {
        assert!(AssistantProviderSettingsRevision::new(0).is_none());
        assert_eq!(AssistantProviderSettingsRevision::new(7).unwrap().get(), 7);
    }

    #[test]
    fn api_key_is_bounded_and_redacted_from_debug() {
        let key = AssistantProviderApiKey::try_new(b"secret-value".to_vec()).unwrap();
        assert_eq!(key.as_bytes(), b"secret-value");
        assert_eq!(format!("{key:?}"), "AssistantProviderApiKey([REDACTED])");
        assert_eq!(
            AssistantProviderApiKey::try_new(Vec::new()),
            Err(AssistantProviderSettingsValueError::InvalidApiKey)
        );
    }

    #[test]
    fn enabled_snapshot_requires_model_and_credential() {
        let result = AssistantProviderSettingsSnapshot::try_new(
            AssistantProviderSettingsRevision::new(1).unwrap(),
            true,
            AssistantProviderBaseUrl::try_new("https://api.openai.com/v1").unwrap(),
            None,
            false,
        );

        assert_eq!(result, Err(AssistantProviderSettingsValueError::InvalidSnapshot));
    }
}
