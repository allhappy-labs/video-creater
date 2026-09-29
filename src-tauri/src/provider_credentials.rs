use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

pub const PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE: &str =
    "com.olhapi.video-creater.provider-credentials";
pub const PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV: &str =
    "VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE";
const ACCEPTANCE_KEYCHAIN_SERVICE_PREFIX: &str = "com.olhapi.video-creater.settings-acceptance.";
pub const MAX_PROVIDER_CREDENTIAL_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderCredentialSource {
    Keychain,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialStatus {
    pub provider: String,
    pub display_name: String,
    pub configured: bool,
    pub source: ProviderCredentialSource,
}

/// A resolved credential deliberately has no `Debug` or `Serialize`
/// implementation so secrets cannot accidentally enter logs or IPC payloads.
pub struct ResolvedProviderCredential {
    provider: &'static str,
    secret: String,
}

impl ResolvedProviderCredential {
    pub fn provider(&self) -> &'static str {
        self.provider
    }

    pub fn source(&self) -> ProviderCredentialSource {
        ProviderCredentialSource::Keychain
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    pub fn into_secret(self) -> String {
        self.secret
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderCredentialError {
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("provider credential cannot be blank")]
    BlankCredential,
    #[error("provider credential exceeds the {MAX_PROVIDER_CREDENTIAL_BYTES}-byte limit")]
    CredentialTooLarge,
    #[error("no credential is configured for provider {0}")]
    MissingCredential(String),
    #[error("secure credential storage is unavailable: {0}")]
    StoreUnavailable(String),
    #[error("secure credential storage failed: {0}")]
    Store(String),
    #[error("stored credential is not valid UTF-8")]
    InvalidStoredCredential,
}

#[derive(Debug, Clone, Copy)]
struct ProviderDefinition {
    provider: &'static str,
    display_name: &'static str,
}

const PROVIDERS: [ProviderDefinition; 7] = [
    ProviderDefinition {
        provider: "fal.ai",
        display_name: "fal.ai",
    },
    ProviderDefinition {
        provider: "replicate",
        display_name: "Replicate",
    },
    ProviderDefinition {
        provider: "openai",
        display_name: "OpenAI",
    },
    ProviderDefinition {
        provider: "xai",
        display_name: "xAI",
    },
    ProviderDefinition {
        provider: "elevenlabs",
        display_name: "ElevenLabs",
    },
    ProviderDefinition {
        provider: "google",
        display_name: "Google",
    },
    ProviderDefinition {
        provider: "minimax",
        display_name: "MiniMax",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialStoreError {
    Unavailable(String),
    Other(String),
}

pub trait CredentialStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError>;
    fn set(&self, account: &str, credential: &[u8]) -> Result<(), CredentialStoreError>;
    fn delete(&self, account: &str) -> Result<(), CredentialStoreError>;
}

pub trait CredentialEnvironment {
    fn get(&self, name: &str) -> Option<String>;
}

struct ProcessEnvironment;

impl CredentialEnvironment for ProcessEnvironment {
    fn get(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
}

fn keychain_service_with(
    environment: &impl CredentialEnvironment,
) -> Result<String, CredentialStoreError> {
    let Some(service) = environment.get(PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV) else {
        return Ok(PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE.to_string());
    };
    let Some(suffix) = service.strip_prefix(ACCEPTANCE_KEYCHAIN_SERVICE_PREFIX) else {
        return Err(CredentialStoreError::Unavailable(
            "provider Keychain service override is outside the acceptance namespace".to_string(),
        ));
    };
    if suffix.len() != 32
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CredentialStoreError::Unavailable(
            "provider Keychain service override suffix is invalid".to_string(),
        ));
    }
    Ok(service)
}

#[derive(Default)]
struct PlatformCredentialStore;

#[cfg(target_os = "macos")]
impl CredentialStore for PlatformCredentialStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError> {
        use security_framework::passwords::get_generic_password;

        let service = keychain_service_with(&ProcessEnvironment)?;
        match get_generic_password(&service, account) {
            Ok(value) => Ok(Some(value)),
            // Security.framework's errSecItemNotFound. Keeping the constant local
            // avoids a second direct dependency solely for this status code.
            Err(error) if error.code() == -25_300 => Ok(None),
            Err(error) => Err(CredentialStoreError::Unavailable(error.to_string())),
        }
    }

    fn set(&self, account: &str, credential: &[u8]) -> Result<(), CredentialStoreError> {
        let service = keychain_service_with(&ProcessEnvironment)?;
        security_framework::passwords::set_generic_password(&service, account, credential)
            .map_err(|error| CredentialStoreError::Other(error.to_string()))
    }

    fn delete(&self, account: &str) -> Result<(), CredentialStoreError> {
        use security_framework::passwords::delete_generic_password;

        let service = keychain_service_with(&ProcessEnvironment)?;
        match delete_generic_password(&service, account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25_300 => Ok(()),
            Err(error) => Err(CredentialStoreError::Other(error.to_string())),
        }
    }
}

#[cfg(target_os = "linux")]
impl CredentialStore for PlatformCredentialStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError> {
        let service = keychain_service_with(&ProcessEnvironment)?;
        linux_secret_service::get(linux_secret_service::Bus::Session, &service, account)
    }

    fn set(&self, account: &str, credential: &[u8]) -> Result<(), CredentialStoreError> {
        let service = keychain_service_with(&ProcessEnvironment)?;
        linux_secret_service::set(
            linux_secret_service::Bus::Session,
            &service,
            account,
            credential,
        )
    }

    fn delete(&self, account: &str) -> Result<(), CredentialStoreError> {
        let service = keychain_service_with(&ProcessEnvironment)?;
        linux_secret_service::delete(linux_secret_service::Bus::Session, &service, account)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl CredentialStore for PlatformCredentialStore {
    fn get(&self, _account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError> {
        Err(CredentialStoreError::Unavailable(
            "secure credential storage is not supported on this platform".to_string(),
        ))
    }

    fn set(&self, _account: &str, _credential: &[u8]) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Unavailable(
            "secure credential storage is not supported on this platform".to_string(),
        ))
    }

    fn delete(&self, _account: &str) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Unavailable(
            "secure credential storage is not supported on this platform".to_string(),
        ))
    }
}

/// Human-readable name of the secure store that holds provider credentials.
/// The serialized source stays `"keychain"` on every platform for wire
/// compatibility; only user-facing copy changes.
pub fn credential_store_display_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS Keychain"
    } else if cfg!(target_os = "linux") {
        "the system keyring"
    } else {
        "secure credential storage"
    }
}

/// Linux provider credentials live in the freedesktop Secret Service
/// (GNOME Keyring, KeePassXC, KWallet's Secret Service bridge) and are
/// addressed with the same service/account pair used for macOS Keychain
/// generic passwords. There is deliberately no plaintext fallback: when the
/// session bus, the Secret Service provider, or an unlocked default
/// collection is missing, every operation reports `Unavailable`.
#[cfg(target_os = "linux")]
mod linux_secret_service {
    use super::CredentialStoreError;
    use secret_service::blocking::{Collection, Item, SecretService};
    use secret_service::{EncryptionType, Error};
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::Duration;

    pub(super) const SERVICE_ATTRIBUTE: &str = "service";
    pub(super) const ACCOUNT_ATTRIBUTE: &str = "account";
    const APPLICATION_ATTRIBUTE: &str = "application";
    const APPLICATION_ID: &str = "com.olhapi.video-creater";
    const CONTENT_TYPE: &str = "text/plain";
    const METHOD_TIMEOUT: Duration = Duration::from_secs(10);
    const READ_TIMEOUT: Duration = Duration::from_secs(20);
    /// Saving may need the provider's unlock prompt, which waits for the user.
    const WRITE_TIMEOUT: Duration = Duration::from_secs(120);

    /// Which session bus to talk to. Production always uses the login session
    /// bus; tests may point at an explicit address.
    #[derive(Clone, Debug)]
    pub(super) enum Bus {
        Session,
        #[cfg_attr(not(test), allow(dead_code))]
        Address(String),
    }

    pub(super) fn get(
        bus: Bus,
        service: &str,
        account: &str,
    ) -> Result<Option<Vec<u8>>, CredentialStoreError> {
        let (service, account) = (service.to_string(), account.to_string());
        bounded("read", READ_TIMEOUT, move || {
            let secret_service = connect(&bus)?;
            let items = secret_service
                .search_items(lookup_attributes(&service, &account))
                .map_err(unavailable)?;
            if let Some(item) = items.unlocked.first() {
                return item.get_secret().map(Some).map_err(unavailable);
            }
            if !items.locked.is_empty() {
                return Err(CredentialStoreError::Unavailable(
                    "the system keyring is locked; unlock it and try again".to_string(),
                ));
            }
            // Some providers hide the items of a locked collection from
            // searches, so an empty result only means "missing" when the
            // collection new credentials are written to is present and unlocked.
            let collection = default_collection(&secret_service)?;
            if collection.is_locked().map_err(unavailable)? {
                return Err(CredentialStoreError::Unavailable(
                    "the system keyring is locked; unlock it and try again".to_string(),
                ));
            }
            Ok(None)
        })
    }

    pub(super) fn set(
        bus: Bus,
        service: &str,
        account: &str,
        credential: &[u8],
    ) -> Result<(), CredentialStoreError> {
        let (service, account, credential) = (
            service.to_string(),
            account.to_string(),
            credential.to_vec(),
        );
        bounded("write", WRITE_TIMEOUT, move || {
            let secret_service = connect(&bus)?;
            let collection = default_collection(&secret_service)?;
            if collection.is_locked().map_err(unavailable)? {
                collection.unlock().map_err(unavailable)?;
            }
            let mut attributes = lookup_attributes(&service, &account);
            attributes.insert(APPLICATION_ATTRIBUTE, APPLICATION_ID);
            let label = format!("Video Creater provider credential ({account})");
            let created = collection
                .create_item(&label, attributes, &credential, true, CONTENT_TYPE)
                .map_err(store_error)?;
            // Remove stale copies another collection may hold so reads cannot
            // resolve an older value than the one just written.
            let duplicates = secret_service
                .search_items(lookup_attributes(&service, &account))
                .map_err(store_error)?;
            for item in duplicates.unlocked.iter() {
                if item.item_path != created.item_path {
                    item.delete().map_err(store_error)?;
                }
            }
            Ok(())
        })
    }

    pub(super) fn delete(
        bus: Bus,
        service: &str,
        account: &str,
    ) -> Result<(), CredentialStoreError> {
        let (service, account) = (service.to_string(), account.to_string());
        bounded("delete", READ_TIMEOUT, move || {
            let secret_service = connect(&bus)?;
            let items = secret_service
                .search_items(lookup_attributes(&service, &account))
                .map_err(unavailable)?;
            if !items.locked.is_empty() {
                return Err(CredentialStoreError::Unavailable(
                    "the system keyring is locked; unlock it and try again".to_string(),
                ));
            }
            items
                .unlocked
                .iter()
                .try_for_each(|item: &Item<'_>| item.delete().map_err(store_error))
        })
    }

    pub(super) fn lookup_attributes<'a>(
        service: &'a str,
        account: &'a str,
    ) -> HashMap<&'a str, &'a str> {
        HashMap::from([(SERVICE_ATTRIBUTE, service), (ACCOUNT_ATTRIBUTE, account)])
    }

    fn connect(bus: &Bus) -> Result<SecretService<'static>, CredentialStoreError> {
        let builder = match bus {
            Bus::Session => zbus::blocking::connection::Builder::session(),
            Bus::Address(address) => zbus::blocking::connection::Builder::address(address.as_str()),
        };
        let connection = builder
            .and_then(|builder| builder.method_timeout(METHOD_TIMEOUT).build())
            .map_err(|error| {
                CredentialStoreError::Unavailable(format!(
                    "the D-Bus session bus is unavailable: {error}"
                ))
            })?;
        SecretService::connect_with_existing(EncryptionType::Dh, connection).map_err(|error| {
            CredentialStoreError::Unavailable(format!(
                "no Secret Service provider is available on the session bus: {error}"
            ))
        })
    }

    fn default_collection<'a>(
        secret_service: &'a SecretService<'a>,
    ) -> Result<Collection<'a>, CredentialStoreError> {
        secret_service
            .get_default_collection()
            .map_err(|error| match error {
                Error::NoResult => CredentialStoreError::Unavailable(
                    "the system keyring has no default collection".to_string(),
                ),
                error => unavailable(error),
            })
    }

    fn unavailable(error: Error) -> CredentialStoreError {
        CredentialStoreError::Unavailable(describe(&error))
    }

    fn store_error(error: Error) -> CredentialStoreError {
        match error {
            Error::Crypto(_) | Error::Zvariant(_) | Error::NoResult => {
                CredentialStoreError::Other(describe(&error))
            }
            error => unavailable(error),
        }
    }

    fn describe(error: &Error) -> String {
        match error {
            Error::Locked => "the system keyring is locked; unlock it and try again".to_string(),
            Error::Prompt => "the system keyring unlock prompt was dismissed".to_string(),
            Error::PromptDisconnected => {
                "the system keyring closed before the unlock prompt completed".to_string()
            }
            Error::Unavailable => {
                "no Secret Service provider or D-Bus session bus was found".to_string()
            }
            error => format!("Secret Service request failed: {error}"),
        }
    }

    /// Runs a Secret Service operation on a dedicated thread so an unresponsive
    /// provider or a pending unlock prompt cannot block the caller forever.
    fn bounded<T, F>(operation: &str, timeout: Duration, run: F) -> Result<T, CredentialStoreError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, CredentialStoreError> + Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("secret-service".to_string())
            .spawn(move || {
                let _ = sender.send(run());
            })
            .map_err(|error| {
                CredentialStoreError::Unavailable(format!(
                    "Secret Service {operation} could not start: {error}"
                ))
            })?;
        receiver.recv_timeout(timeout).unwrap_or_else(|error| {
            Err(CredentialStoreError::Unavailable(format!(
                "Secret Service {operation} did not complete: {error}"
            )))
        })
    }
}

pub fn list_provider_credential_statuses() -> Vec<ProviderCredentialStatus> {
    list_provider_credential_statuses_with_store(&PlatformCredentialStore)
}

pub fn provider_credential_status(
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    provider_credential_status_with_store(&PlatformCredentialStore, provider)
}

pub fn set_provider_credential(
    provider: &str,
    credential: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    set_provider_credential_with_store(&PlatformCredentialStore, provider, credential)
}

pub fn delete_provider_credential(
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    delete_provider_credential_with_store(&PlatformCredentialStore, provider)
}

pub fn resolve_provider_credential(
    provider: &str,
) -> Result<ResolvedProviderCredential, ProviderCredentialError> {
    resolve_provider_credential_with_store(&PlatformCredentialStore, provider)
}

pub fn list_provider_credential_statuses_with_store(
    store: &impl CredentialStore,
) -> Vec<ProviderCredentialStatus> {
    list_with(store)
}

pub fn provider_credential_status_with_store(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    status_with(store, provider)
}

pub fn set_provider_credential_with_store(
    store: &impl CredentialStore,
    provider: &str,
    credential: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    set_with(store, provider, credential)
}

pub fn delete_provider_credential_with_store(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    delete_with(store, provider)
}

pub fn resolve_provider_credential_with_store(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ResolvedProviderCredential, ProviderCredentialError> {
    resolve_with(store, provider)
}

pub fn is_supported_provider(provider: &str) -> bool {
    definition(provider.trim()).is_ok()
}

pub fn supported_provider_count() -> usize {
    PROVIDERS.len()
}

pub fn supported_provider_ids() -> impl Iterator<Item = &'static str> {
    PROVIDERS.iter().map(|definition| definition.provider)
}

fn list_with(store: &impl CredentialStore) -> Vec<ProviderCredentialStatus> {
    PROVIDERS
        .iter()
        .map(|definition| status_for_definition(store, definition))
        .collect()
}

fn status_with(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    let definition = definition(provider)?;
    Ok(status_for_definition(store, definition))
}

fn status_for_definition(
    store: &impl CredentialStore,
    definition: &ProviderDefinition,
) -> ProviderCredentialStatus {
    let source = match store.get(definition.provider) {
        Ok(Some(value)) if valid_stored_credential(&value) => ProviderCredentialSource::Keychain,
        Ok(Some(_)) | Ok(None) => ProviderCredentialSource::Missing,
        Err(_) => ProviderCredentialSource::Unavailable,
    };
    ProviderCredentialStatus {
        provider: definition.provider.to_string(),
        display_name: definition.display_name.to_string(),
        configured: source == ProviderCredentialSource::Keychain,
        source,
    }
}

fn set_with(
    store: &impl CredentialStore,
    provider: &str,
    credential: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    let definition = definition(provider)?;
    validate_credential(credential)?;
    store
        .set(definition.provider, credential.as_bytes())
        .map_err(map_store_error)?;
    Ok(status_for_definition(store, definition))
}

fn delete_with(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ProviderCredentialStatus, ProviderCredentialError> {
    let definition = definition(provider)?;
    store.delete(definition.provider).map_err(map_store_error)?;
    Ok(status_for_definition(store, definition))
}

fn resolve_with(
    store: &impl CredentialStore,
    provider: &str,
) -> Result<ResolvedProviderCredential, ProviderCredentialError> {
    let definition = definition(provider)?;
    let keychain_result = store.get(definition.provider);
    if let Ok(Some(value)) = &keychain_result {
        let secret = String::from_utf8(value.clone())
            .map_err(|_| ProviderCredentialError::InvalidStoredCredential)?;
        validate_credential(&secret)?;
        return Ok(ResolvedProviderCredential {
            provider: definition.provider,
            secret,
        });
    }
    match keychain_result {
        Err(error) => Err(map_store_error(error)),
        Ok(Some(_)) => Err(ProviderCredentialError::InvalidStoredCredential),
        Ok(None) => Err(ProviderCredentialError::MissingCredential(
            definition.provider.to_string(),
        )),
    }
}

fn definition(provider: &str) -> Result<&'static ProviderDefinition, ProviderCredentialError> {
    PROVIDERS
        .iter()
        .find(|definition| definition.provider == provider)
        .ok_or_else(|| ProviderCredentialError::UnsupportedProvider(provider.to_string()))
}

fn validate_credential(credential: &str) -> Result<(), ProviderCredentialError> {
    if credential.trim().is_empty() {
        return Err(ProviderCredentialError::BlankCredential);
    }
    if credential.len() > MAX_PROVIDER_CREDENTIAL_BYTES {
        return Err(ProviderCredentialError::CredentialTooLarge);
    }
    Ok(())
}

fn valid_stored_credential(credential: &[u8]) -> bool {
    std::str::from_utf8(credential).is_ok_and(|value| validate_credential(value).is_ok())
}

fn map_store_error(error: CredentialStoreError) -> ProviderCredentialError {
    match error {
        CredentialStoreError::Unavailable(message) => {
            ProviderCredentialError::StoreUnavailable(message)
        }
        CredentialStoreError::Other(message) => ProviderCredentialError::Store(message),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    mod disposable_keyring;

    use super::*;
    use serde_json::Value;
    use std::{cell::RefCell, collections::HashMap};

    fn assert_status_payload_secret_free(value: &Value, fixture_secret: &str) {
        match value {
            Value::Object(entries) => {
                for (key, child) in entries {
                    let normalized = key.to_ascii_lowercase();
                    assert!(
                        !matches!(
                            normalized.as_str(),
                            "secret" | "token" | "apikey" | "credentialvalue"
                        ),
                        "secret-shaped key was serialized: {key}"
                    );
                    assert_status_payload_secret_free(child, fixture_secret);
                }
            }
            Value::Array(values) => {
                for child in values {
                    assert_status_payload_secret_free(child, fixture_secret);
                }
            }
            Value::String(value) => assert!(
                !value.contains(fixture_secret),
                "fixture credential was serialized"
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    #[derive(Default)]
    struct FakeStore {
        credentials: RefCell<HashMap<String, Vec<u8>>>,
        unavailable: bool,
    }

    impl CredentialStore for FakeStore {
        fn get(&self, account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError> {
            if self.unavailable {
                return Err(CredentialStoreError::Unavailable("locked".to_string()));
            }
            Ok(self.credentials.borrow().get(account).cloned())
        }

        fn set(&self, account: &str, credential: &[u8]) -> Result<(), CredentialStoreError> {
            if self.unavailable {
                return Err(CredentialStoreError::Unavailable("locked".to_string()));
            }
            self.credentials
                .borrow_mut()
                .insert(account.to_string(), credential.to_vec());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), CredentialStoreError> {
            if self.unavailable {
                return Err(CredentialStoreError::Unavailable("locked".to_string()));
            }
            self.credentials.borrow_mut().remove(account);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeEnvironment(HashMap<String, String>);

    impl CredentialEnvironment for FakeEnvironment {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    #[test]
    fn provider_definitions_are_complete_and_canonical() {
        let expected = [
            ("fal.ai", "fal.ai"),
            ("replicate", "Replicate"),
            ("openai", "OpenAI"),
            ("xai", "xAI"),
            ("elevenlabs", "ElevenLabs"),
            ("google", "Google"),
            ("minimax", "MiniMax"),
        ];
        assert_eq!(supported_provider_count(), expected.len());
        assert_eq!(
            supported_provider_ids().collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(provider, _)| *provider)
                .collect::<Vec<_>>()
        );
        let statuses = list_with(&FakeStore::default());
        assert_eq!(statuses.len(), expected.len());
        for (status, (provider, display_name)) in statuses.iter().zip(expected) {
            assert_eq!(status.provider, provider);
            assert_eq!(status.display_name, display_name);
            assert!(!status.configured);
            assert_eq!(status.source, ProviderCredentialSource::Missing);
        }
    }

    #[test]
    fn acceptance_keychain_service_override_is_strict_and_namespaced() {
        let valid = format!(
            "com.olhapi.video-creater.settings-acceptance.{}",
            "0123456789abcdef0123456789abcdef"
        );
        let environment = FakeEnvironment(HashMap::from([(
            PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV.to_string(),
            valid.clone(),
        )]));
        assert_eq!(
            keychain_service_with(&environment).expect("valid service"),
            valid
        );

        for invalid in [
            "com.olhapi.video-creater.provider-credentials",
            "com.olhapi.video-creater.settings-acceptance.weak",
            "com.olhapi.video-creater.settings-acceptance.0123456789ABCDEF0123456789ABCDEF",
            "other.settings-acceptance.0123456789abcdef0123456789abcdef",
        ] {
            let environment = FakeEnvironment(HashMap::from([(
                PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV.to_string(),
                invalid.to_string(),
            )]));
            assert!(keychain_service_with(&environment).is_err());
        }
        assert_eq!(
            keychain_service_with(&FakeEnvironment::default()).expect("default service"),
            PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE
        );
    }

    #[test]
    fn missing_keychain_item_stays_missing_even_when_process_environment_has_a_key() {
        let fixture_secret = "provider-list-fixture-secret-c2dd";
        std::env::set_var("OPENAI_API_KEY", fixture_secret);

        let status = status_with(&FakeStore::default(), "openai").expect("missing status");
        std::env::remove_var("OPENAI_API_KEY");
        let payload = serde_json::to_value(&status).expect("serialize provider status");

        assert_status_payload_secret_free(&payload, fixture_secret);
        assert_eq!(status.source, ProviderCredentialSource::Missing);
        assert!(!status.configured);
        assert!(payload.get("envVar").is_none());
    }

    #[test]
    fn keychain_wins_over_environment() {
        let store = FakeStore::default();
        store
            .set("openai", b"keychain-secret")
            .expect("seed keychain");
        std::env::set_var("OPENAI_API_KEY", "environment-secret");
        let resolved = resolve_with(&store, "openai").expect("resolve");
        std::env::remove_var("OPENAI_API_KEY");
        assert_eq!(resolved.secret(), "keychain-secret");
        assert_eq!(resolved.provider(), "openai");
        assert_eq!(resolved.source(), ProviderCredentialSource::Keychain);
    }

    #[test]
    fn resolver_never_returns_an_environment_credential() {
        std::env::set_var("FAL_KEY", "environment-secret");

        let error = match resolve_with(&FakeStore::default(), "fal.ai") {
            Err(error) => error,
            Ok(_) => panic!("environment credential must not resolve"),
        };
        std::env::remove_var("FAL_KEY");

        assert_eq!(
            error,
            ProviderCredentialError::MissingCredential("fal.ai".to_string())
        );
    }

    #[test]
    fn status_distinguishes_missing_and_unavailable() {
        let missing = status_with(&FakeStore::default(), "google").expect("missing status");
        assert_eq!(missing.source, ProviderCredentialSource::Missing);

        let unavailable = status_with(
            &FakeStore {
                unavailable: true,
                ..FakeStore::default()
            },
            "google",
        )
        .expect("unavailable status");
        assert_eq!(unavailable.source, ProviderCredentialSource::Unavailable);
    }

    #[test]
    fn set_and_delete_round_trip_without_exposing_secret_in_status() {
        let store = FakeStore::default();
        let secret = "highly-sensitive-test-secret";
        let stored = set_with(&store, "replicate", secret).expect("set");
        assert_eq!(stored.source, ProviderCredentialSource::Keychain);
        assert!(stored.configured);
        let serialized = serde_json::to_string(&stored).expect("serialize status");
        assert!(!serialized.contains(secret));
        assert_status_payload_secret_free(
            &serde_json::to_value(&stored).expect("serialize set payload"),
            secret,
        );
        assert_eq!(
            serialized,
            r#"{"provider":"replicate","displayName":"Replicate","configured":true,"source":"keychain"}"#
        );

        let deleted = delete_with(&store, "replicate").expect("delete");
        assert_eq!(deleted.source, ProviderCredentialSource::Missing);
        assert!(!deleted.configured);
        assert_status_payload_secret_free(
            &serde_json::to_value(&deleted).expect("serialize delete payload"),
            secret,
        );
    }

    #[test]
    fn delete_stays_missing_when_environment_contains_a_credential() {
        let fixture_secret = "provider-delete-fixture-secret-e8a1";
        let store = FakeStore::default();
        store.set("xai", b"stored-secret").expect("seed keychain");
        std::env::set_var("XAI_API_KEY", fixture_secret);
        let status = delete_with(&store, "xai").expect("delete");
        std::env::remove_var("XAI_API_KEY");
        assert_eq!(status.source, ProviderCredentialSource::Missing);
        assert!(!status.configured);
        let payload = serde_json::to_value(&status).expect("serialize deleted status payload");
        assert_status_payload_secret_free(&payload, fixture_secret);
        assert!(payload.get("envVar").is_none());
    }

    #[test]
    fn rejects_unsupported_blank_and_oversized_values() {
        let store = FakeStore::default();
        assert_eq!(
            set_with(&store, "unknown", "secret"),
            Err(ProviderCredentialError::UnsupportedProvider(
                "unknown".to_string()
            ))
        );
        for blank in ["", " ", "\n\t"] {
            assert_eq!(
                set_with(&store, "openai", blank),
                Err(ProviderCredentialError::BlankCredential)
            );
        }
        let fixture_secret = "oversized-provider-fixture-secret-6b9e";
        let oversized = fixture_secret
            .repeat((MAX_PROVIDER_CREDENTIAL_BYTES / fixture_secret.len()).saturating_add(2));
        let error =
            set_with(&store, "openai", &oversized).expect_err("oversized credential must fail");
        assert_eq!(error, ProviderCredentialError::CredentialTooLarge);
        assert!(!error.to_string().contains(fixture_secret));
    }

    #[test]
    fn missing_and_unavailable_resolution_are_explicit_and_secret_free() {
        let missing = match resolve_with(&FakeStore::default(), "minimax") {
            Err(error) => error,
            Ok(_) => panic!("missing must fail"),
        };
        assert_eq!(
            missing,
            ProviderCredentialError::MissingCredential("minimax".to_string())
        );
        let unavailable = match resolve_with(
            &FakeStore {
                unavailable: true,
                ..FakeStore::default()
            },
            "minimax",
        ) {
            Err(error) => error,
            Ok(_) => panic!("unavailable must fail"),
        };
        assert_eq!(
            unavailable,
            ProviderCredentialError::StoreUnavailable("locked".to_string())
        );
        let rendered = format!("{missing}; {unavailable}");
        assert!(!rendered.contains("secret"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_secret_service_without_a_bus_is_unavailable_never_missing() {
        use linux_secret_service::{delete, get, set, Bus};

        let bus = || Bus::Address("unix:path=/nonexistent/video-creater-test-bus".to_string());
        let service = PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE;
        assert!(matches!(
            get(bus(), service, "openai"),
            Err(CredentialStoreError::Unavailable(_))
        ));
        assert!(matches!(
            set(bus(), service, "openai", b"secret"),
            Err(CredentialStoreError::Unavailable(_))
        ));
        assert!(matches!(
            delete(bus(), service, "openai"),
            Err(CredentialStoreError::Unavailable(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a session bus without an org.freedesktop.secrets provider"]
    fn linux_session_bus_without_secret_service_provider_is_unavailable() {
        let error = PlatformCredentialStore
            .get("openai")
            .expect_err("a bus without a Secret Service provider must not read as missing");
        assert!(
            matches!(error, CredentialStoreError::Unavailable(_)),
            "{error:?}"
        );
        assert!(matches!(
            PlatformCredentialStore.set("openai", b"secret"),
            Err(CredentialStoreError::Unavailable(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires GNOME Keyring on the session bus; uses a collection it creates"]
    fn linux_secret_service_replaces_and_deletes_an_isolated_account() {
        struct IsolatedSecretCleanup(String);

        impl Drop for IsolatedSecretCleanup {
            fn drop(&mut self) {
                let _ = PlatformCredentialStore.delete(&self.0);
            }
        }

        let _keyring = disposable_keyring::DisposableDefaultCollection::create();
        let service = keychain_service_with(&ProcessEnvironment)
            .expect("acceptance Secret Service namespace override");
        assert_ne!(
            service, PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE,
            "set {PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV} to an acceptance namespace so real credentials are never touched"
        );
        let store = PlatformCredentialStore;
        let cleanup =
            IsolatedSecretCleanup(format!("__video_creater_test__{}", uuid::Uuid::new_v4()));
        let first_secret = b"video-creater-secret-service-e2e-first";
        let replacement_secret = b"video-creater-secret-service-e2e-replacement";
        assert!(store.get(&cleanup.0).expect("read before write").is_none());

        store
            .set(&cleanup.0, first_secret)
            .expect("write first isolated credential");
        assert!(
            store.get(&cleanup.0).expect("read first").as_deref() == Some(&first_secret[..]),
            "first isolated credential was not persisted"
        );

        store
            .set(&cleanup.0, replacement_secret)
            .expect("replace isolated credential");
        assert!(
            store.get(&cleanup.0).expect("read replacement").as_deref()
                == Some(&replacement_secret[..]),
            "replacement credential was not persisted"
        );
        let secret_service =
            secret_service::blocking::SecretService::connect(secret_service::EncryptionType::Dh)
                .expect("connect to Secret Service");
        let matches = secret_service
            .search_items(linux_secret_service::lookup_attributes(
                &service, &cleanup.0,
            ))
            .expect("search isolated credential");
        assert_eq!(
            matches.unlocked.len(),
            1,
            "replacement must not duplicate items"
        );
        assert!(matches.locked.is_empty());
        let attributes = matches.unlocked[0]
            .get_attributes()
            .expect("item attributes");
        assert_eq!(
            attributes.get("service").map(String::as_str),
            Some(service.as_str())
        );
        assert_eq!(attributes.get("account"), Some(&cleanup.0));

        let status = set_with(&store, "openai", "provider-status-secret-service-e2e");
        let _ = store.delete("openai");
        let status = status.expect("set provider credential through Secret Service");
        assert!(status.configured);
        assert_eq!(status.source, ProviderCredentialSource::Keychain);

        store
            .delete(&cleanup.0)
            .expect("delete isolated credential");
        assert!(
            store.get(&cleanup.0).expect("read after delete").is_none(),
            "isolated credential remained after delete"
        );
        store
            .delete(&cleanup.0)
            .expect("deleting a missing credential succeeds");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires GNOME Keyring on the session bus; locks a collection it creates"]
    fn linux_locked_secret_service_is_unavailable_never_missing() {
        let service = keychain_service_with(&ProcessEnvironment)
            .expect("acceptance Secret Service namespace override");
        assert_ne!(service, PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE);
        // Locks only a collection this test creates; the previous default returns afterwards.
        let _keyring = disposable_keyring::DisposableDefaultCollection::create();
        let account = format!("__video_creater_test__{}", uuid::Uuid::new_v4());
        let store = PlatformCredentialStore;
        store
            .set(&account, b"locked-fixture")
            .expect("seed credential");

        let secret_service =
            secret_service::blocking::SecretService::connect(secret_service::EncryptionType::Dh)
                .expect("connect to Secret Service");
        secret_service
            .get_default_collection()
            .expect("default collection")
            .lock()
            .expect("lock default collection");

        for result in [store.get(&account).map(|_| ()), store.delete(&account)] {
            let error = result.expect_err("locked collection must not read as missing");
            assert!(
                matches!(error, CredentialStoreError::Unavailable(_)),
                "{error:?}"
            );
        }
        assert_eq!(
            status_with(&store, "openai").expect("status").source,
            ProviderCredentialSource::Unavailable
        );
        let started = std::time::Instant::now();
        let error = store
            .set(&account, b"replacement")
            .expect_err("locked collection without an unlock prompt must fail");
        assert!(
            matches!(error, CredentialStoreError::Unavailable(_)),
            "{error:?}"
        );
        eprintln!("locked set failed after {:?}: {error:?}", started.elapsed());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires an unlocked macOS login Keychain"]
    fn macos_keychain_replaces_and_deletes_an_isolated_account() {
        struct IsolatedKeychainCleanup(String);

        impl Drop for IsolatedKeychainCleanup {
            fn drop(&mut self) {
                let _ = PlatformCredentialStore.delete(&self.0);
            }
        }

        let store = PlatformCredentialStore;
        let cleanup =
            IsolatedKeychainCleanup(format!("__video_creater_test__{}", uuid::Uuid::new_v4()));
        let first_secret = b"video-creater-keychain-e2e-first";
        let replacement_secret = b"video-creater-keychain-e2e-replacement";
        let _ = store.delete(&cleanup.0);
        store
            .set(&cleanup.0, first_secret)
            .expect("write first isolated credential");
        let first_stored = store
            .get(&cleanup.0)
            .expect("read first isolated credential")
            .expect("credential exists");
        assert!(
            first_stored == first_secret,
            "first isolated credential was not persisted"
        );

        store
            .set(&cleanup.0, replacement_secret)
            .expect("replace isolated credential");
        let replacement_stored = store
            .get(&cleanup.0)
            .expect("read replacement isolated credential")
            .expect("replacement credential exists");
        assert!(
            replacement_stored == replacement_secret,
            "replacement credential was not persisted"
        );

        store
            .delete(&cleanup.0)
            .expect("delete isolated credential");
        assert!(
            store
                .get(&cleanup.0)
                .expect("read after isolated credential delete")
                .is_none(),
            "isolated credential remained after delete"
        );
    }
}
