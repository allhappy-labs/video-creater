use std::fs;
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

pub struct SessionStore {
    path: PathBuf,
    origin: String,
    lifetime_seconds: u64,
    state: Mutex<SessionFile>,
}

impl SessionStore {
    pub fn load(path: PathBuf, origin: &str, lifetime_seconds: u64) -> Result<Self, SessionError> {
        let state = if path.exists() {
            serde_json::from_slice(&fs::read(&path).map_err(|_| SessionError::Io)?)
                .map_err(|_| SessionError::Io)?
        } else {
            SessionFile::default()
        };
        Ok(Self {
            path,
            origin: origin.trim_end_matches('/').to_owned(),
            lifetime_seconds,
            state: Mutex::new(state),
        })
    }

    pub fn issue(
        &self,
        display_name: &str,
        identity: Option<&str>,
        now: u64,
    ) -> Result<IssuedSession, SessionError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let credential = random_secret();
        let csrf_token = random_secret();
        let mut state = self.state.lock().map_err(|_| SessionError::Io)?;
        state.sessions.push(StoredSession {
            id: session_id.clone(),
            credential_hash: hash(&credential),
            csrf_hash: hash(&csrf_token),
            previous_csrf_hashes: Vec::new(),
            display_name: display_name.chars().take(100).collect(),
            identity: identity.map(str::to_owned),
            created_at: now,
            expires_at: now.saturating_add(self.lifetime_seconds),
            revoked: false,
        });
        self.persist(&state)?;
        Ok(IssuedSession {
            session_id,
            cookie_header: format!(
                "vc_session={credential}; Path=/; Secure; HttpOnly; SameSite=Strict"
            ),
            credential,
            csrf_token,
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
        let mut state = self.state.lock().map_err(|_| SessionError::Io)?;
        let session = state
            .sessions
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or(SessionError::Invalid)?;
        session.revoked = true;
        self.persist(&state)
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
        let current = self.authenticate(credential, now)?;
        self.revoke(&current.session_id)?;
        self.issue(&current.display_name, current.identity.as_deref(), now)
    }

    pub fn refresh_csrf(&self, session_id: &str) -> Result<String, SessionError> {
        let csrf_token = random_secret();
        let mut state = self.state.lock().map_err(|_| SessionError::Io)?;
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
        self.persist(&state)?;
        Ok(csrf_token)
    }

    pub fn revoke_all(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Io)?;
        for session in &mut state.sessions {
            session.revoked = true;
        }
        self.persist(&state)
    }

    fn persist(&self, state: &SessionFile) -> Result<(), SessionError> {
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
            file.write_all(&serde_json::to_vec_pretty(state).map_err(|_| SessionError::Io)?)
                .map_err(|_| SessionError::Io)?;
            file.sync_all().map_err(|_| SessionError::Io)?;
        }
        #[cfg(not(unix))]
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(state).map_err(|_| SessionError::Io)?,
        )
        .map_err(|_| SessionError::Io)?;
        fs::rename(temporary, &self.path).map_err(|_| SessionError::Io)
    }
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
