use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    Io,
    Invalid,
    Expired,
    Revoked,
    Origin,
    Csrf,
    Identity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSession {
    id: String,
    credential_hash: String,
    csrf_hash: String,
    #[serde(default)]
    previous_csrf_hashes: Vec<String>,
    display_name: String,
    identity: Option<String>,
    created_at: u64,
    expires_at: u64,
    revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionFile {
    sessions: Vec<StoredSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub session_id: String,
    pub display_name: String,
    pub identity: Option<String>,
    pub expires_at: u64,
    pub created_at: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone)]
pub struct IssuedSession {
    pub session_id: String,
    pub credential: String,
    pub csrf_token: String,
    pub cookie_header: String,
}

const MAX_LIVE_SESSIONS: usize = 1_024;
const MAX_SESSION_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SESSION_IDENTITY_BYTES: usize = 1_024;

pub struct SessionStore {
    path: PathBuf,
    origin: String,
    lifetime_seconds: u64,
    state: Mutex<SessionFile>,
    authorization_changes: tokio::sync::watch::Sender<()>,
}

impl SessionStore {
    pub fn load(path: PathBuf, origin: &str, lifetime_seconds: u64) -> Result<Self, SessionError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| SessionError::Io)?
            .as_secs();
        Self::load_at(path, origin, lifetime_seconds, now)
    }

    /// Load with an explicit clock for host bootstrap and deterministic expiry handling.
    /// Cleanup is in memory; a failed load never rewrites the existing credential file.
    pub fn load_at(
        path: PathBuf,
        origin: &str,
        lifetime_seconds: u64,
        now: u64,
    ) -> Result<Self, SessionError> {
        let state = if path.exists() {
            let file = fs::File::open(&path).map_err(|_| SessionError::Io)?;
            if file.metadata().map_err(|_| SessionError::Io)?.len() > MAX_SESSION_FILE_BYTES as u64
            {
                return Err(SessionError::Io);
            }
            let mut bytes = Vec::new();
            file.take(MAX_SESSION_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| SessionError::Io)?;
            if bytes.len() > MAX_SESSION_FILE_BYTES {
                return Err(SessionError::Io);
            }
            let mut state: SessionFile =
                serde_json::from_slice(&bytes).map_err(|_| SessionError::Io)?;
            prune_invalid_sessions(&mut state, now);
            validate_capacity(&state)?;
            state
        } else {
            SessionFile::default()
        };
        Ok(Self {
            path,
            origin: origin.trim_end_matches('/').to_owned(),
            lifetime_seconds,
            state: Mutex::new(state),
            authorization_changes: tokio::sync::watch::channel(()).0,
        })
    }

    pub fn issue(
        &self,
        display_name: &str,
        identity: Option<&str>,
        now: u64,
    ) -> Result<IssuedSession, SessionError> {
        self.transact(|state| {
            prune_invalid_sessions(state, now);
            append_session(state, display_name, identity, now, self.lifetime_seconds)
        })
    }

    pub fn authenticate(&self, credential: &str, now: u64) -> Result<SessionView, SessionError> {
        let digest = hash(credential);
        let state = self.state.lock().map_err(|_| SessionError::Io)?;
        let session = state
            .sessions
            .iter()
            .find(|item| constant_eq(item.credential_hash.as_bytes(), digest.as_bytes()))
            .ok_or(SessionError::Invalid)?;
        if session.revoked {
            return Err(SessionError::Revoked);
        }
        if now > session.expires_at {
            return Err(SessionError::Expired);
        }
        Ok(view(session))
    }

    pub fn authenticate_for_identity(
        &self,
        credential: &str,
        identity: &str,
        now: u64,
    ) -> Result<SessionView, SessionError> {
        let session = self.authenticate(credential, now)?;
        if session.identity.as_deref() != Some(identity) {
            return Err(SessionError::Identity);
        }
        Ok(session)
    }

    /// Open event streams must follow revocation and expiry after their handshake.
    pub fn is_active(&self, session_id: &str, now: u64) -> bool {
        self.state.lock().is_ok_and(|state| {
            state.sessions.iter().any(|session| {
                session.id == session_id && !session.revoked && now <= session.expires_at
            })
        })
    }

    pub fn authorization_changes(&self) -> tokio::sync::watch::Receiver<()> {
        self.authorization_changes.subscribe()
    }

    pub fn authorize_mutation(
        &self,
        credential: &str,
        csrf: &str,
        origin: &str,
        now: u64,
    ) -> Result<SessionView, SessionError> {
        if origin != self.origin {
            return Err(SessionError::Origin);
        }
        let view = self.authenticate(credential, now)?;
        let state = self.state.lock().map_err(|_| SessionError::Io)?;
        let session = state
            .sessions
            .iter()
            .find(|item| item.id == view.session_id)
            .ok_or(SessionError::Invalid)?;
        let csrf_hash = hash(csrf);
        if !constant_eq(session.csrf_hash.as_bytes(), csrf_hash.as_bytes())
            && !session
                .previous_csrf_hashes
                .iter()
                .any(|candidate| constant_eq(candidate.as_bytes(), csrf_hash.as_bytes()))
        {
            return Err(SessionError::Csrf);
        }
        Ok(view)
    }

    pub fn list(&self) -> Vec<SessionView> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sessions
            .iter()
            .map(view)
            .collect()
    }

    pub fn revoke(&self, id: &str) -> Result<(), SessionError> {
        self.transact(|state| {
            let session = state
                .sessions
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or(SessionError::Invalid)?;
            session.revoked = true;
            Ok(())
        })
    }

    pub fn logout(&self, credential: &str) -> Result<(), SessionError> {
        let digest = hash(credential);
        let id = self
            .state
            .lock()
            .map_err(|_| SessionError::Io)?
            .sessions
            .iter()
            .find(|item| constant_eq(item.credential_hash.as_bytes(), digest.as_bytes()))
            .map(|item| item.id.clone())
            .ok_or(SessionError::Invalid)?;
        self.revoke(&id)
    }

    pub fn rotate(&self, credential: &str, now: u64) -> Result<IssuedSession, SessionError> {
        let digest = hash(credential);
        self.transact(|state| {
            // Authentication and replacement happen under one state lock. Failed persistence
            // cannot revoke the old credential or expose the replacement credential in memory.
            let current = state
                .sessions
                .iter()
                .find(|item| constant_eq(item.credential_hash.as_bytes(), digest.as_bytes()))
                .ok_or(SessionError::Invalid)?;
            if current.revoked {
                return Err(SessionError::Revoked);
            }
            if now > current.expires_at {
                return Err(SessionError::Expired);
            }
            let current = current.clone();
            prune_invalid_sessions(state, now);
            state
                .sessions
                .iter_mut()
                .find(|item| item.id == current.id)
                .ok_or(SessionError::Invalid)?
                .revoked = true;
            append_session(
                state,
                &current.display_name,
                current.identity.as_deref(),
                now,
                self.lifetime_seconds,
            )
        })
    }

    pub fn refresh_csrf(&self, session_id: &str) -> Result<String, SessionError> {
        self.transact(|state| {
            let csrf_token = random_secret();
            let session = state
                .sessions
                .iter_mut()
                .find(|item| item.id == session_id)
                .ok_or(SessionError::Invalid)?;
            session
                .previous_csrf_hashes
                .insert(0, session.csrf_hash.clone());
            session.previous_csrf_hashes.truncate(4);
            session.csrf_hash = hash(&csrf_token);
            Ok(csrf_token)
        })
    }

    pub fn revoke_all(&self) -> Result<(), SessionError> {
        self.transact(|state| {
            for session in &mut state.sessions {
                session.revoked = true;
            }
            Ok(())
        })
    }

    fn transact<T>(
        &self,
        change: impl FnOnce(&mut SessionFile) -> Result<T, SessionError>,
    ) -> Result<T, SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Io)?;
        let mut next = state.clone();
        let value = change(&mut next)?;
        self.persist(&next)?;
        *state = next;
        self.authorization_changes.send_replace(());
        Ok(value)
    }

    fn persist(&self, state: &SessionFile) -> Result<(), SessionError> {
        validate_capacity(state)?;
        let bytes = serde_json::to_vec_pretty(state).map_err(|_| SessionError::Io)?;
        if bytes.len() > MAX_SESSION_FILE_BYTES {
            return Err(SessionError::Io);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| SessionError::Io)?;
        }
        let temporary = self.path.with_extension("tmp");
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| SessionError::Io)?;
            file.write_all(&bytes).map_err(|_| SessionError::Io)?;
            file.sync_all().map_err(|_| SessionError::Io)?;
        }
        #[cfg(not(unix))]
        fs::write(&temporary, &bytes).map_err(|_| SessionError::Io)?;
        fs::rename(temporary, &self.path).map_err(|_| SessionError::Io)
    }
}

fn validate_capacity(state: &SessionFile) -> Result<(), SessionError> {
    if state
        .sessions
        .iter()
        .filter(|session| !session.revoked)
        .count()
        > MAX_LIVE_SESSIONS
    {
        return Err(SessionError::Io);
    }
    Ok(())
}

fn prune_invalid_sessions(state: &mut SessionFile, now: u64) {
    // At the exact expiry timestamp the existing authentication contract still accepts it.
    state
        .sessions
        .retain(|session| !session.revoked && now <= session.expires_at);
}

fn append_session(
    state: &mut SessionFile,
    display_name: &str,
    identity: Option<&str>,
    now: u64,
    lifetime_seconds: u64,
) -> Result<IssuedSession, SessionError> {
    if state
        .sessions
        .iter()
        .filter(|session| !session.revoked)
        .count()
        >= MAX_LIVE_SESSIONS
        || identity.is_some_and(|value| value.len() > MAX_SESSION_IDENTITY_BYTES)
    {
        return Err(SessionError::Io);
    }
    let session_id = uuid::Uuid::new_v4().to_string();
    let credential = random_secret();
    let csrf_token = random_secret();
    state.sessions.push(StoredSession {
        id: session_id.clone(),
        credential_hash: hash(&credential),
        csrf_hash: hash(&csrf_token),
        previous_csrf_hashes: Vec::new(),
        display_name: display_name.chars().take(100).collect(),
        identity: identity.map(str::to_owned),
        created_at: now,
        expires_at: now.saturating_add(lifetime_seconds),
        revoked: false,
    });
    Ok(IssuedSession {
        session_id,
        cookie_header: format!(
            "vc_session={credential}; Path=/; Secure; HttpOnly; SameSite=Strict"
        ),
        credential,
        csrf_token,
    })
}

fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn constant_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}
fn view(session: &StoredSession) -> SessionView {
    SessionView {
        session_id: session.id.clone(),
        display_name: session.display_name.clone(),
        identity: session.identity.clone(),
        expires_at: session.expires_at,
        created_at: session.created_at,
        revoked: session.revoked,
    }
}

#[cfg(test)]
#[path = "session_retention_tests.rs"]
mod retention_tests;
