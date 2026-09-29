use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAccess {
    Media,
    Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceGrant {
    pub session_id: String,
    pub project_id: String,
    pub access: ResourceAccess,
    pub relative_path: String,
    pub display_name: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedTicket {
    pub token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketStatus {
    Valid,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTicket {
    pub status: TicketStatus,
    pub grant: ResourceGrant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceTicketError {
    Unknown,
    Scope,
    Expired,
}

pub struct ResourceTicketStore {
    ttl_seconds: u64,
    refresh_grace_seconds: u64,
    tickets: Mutex<HashMap<String, ResourceGrant>>,
}

impl ResourceTicketStore {
    pub fn new(ttl_seconds: u64, refresh_grace_seconds: u64) -> Self {
        Self {
            ttl_seconds: ttl_seconds.max(1),
            refresh_grace_seconds,
            tickets: Mutex::new(HashMap::new()),
        }
    }

    pub fn mint(
        &self,
        session_id: &str,
        project_id: &str,
        access: ResourceAccess,
        relative_path: &str,
        display_name: &str,
        now: u64,
    ) -> IssuedTicket {
        let grant = ResourceGrant {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            access,
            relative_path: relative_path.to_owned(),
            display_name: display_name.to_owned(),
            expires_at: now.saturating_add(self.ttl_seconds),
        };
        let mut tickets = self
            .tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tickets.retain(|_, ticket| {
            now <= ticket.expires_at.saturating_add(self.refresh_grace_seconds)
        });
        mint_locked(&mut tickets, grant)
    }

    pub fn validate(
        &self,
        token: &str,
        session_id: &str,
        access: ResourceAccess,
        now: u64,
    ) -> Result<ValidatedTicket, ResourceTicketError> {
        validate_token(token)?;
        let tickets = self
            .tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let grant = tickets
            .get(token)
            .cloned()
            .ok_or(ResourceTicketError::Unknown)?;
        if grant.session_id != session_id || grant.access != access {
            return Err(ResourceTicketError::Scope);
        }
        Ok(ValidatedTicket {
            status: if now > grant.expires_at {
                TicketStatus::Expired
            } else {
                TicketStatus::Valid
            },
            grant,
        })
    }

    pub fn refresh(
        &self,
        token: &str,
        session_id: &str,
        access: ResourceAccess,
        now: u64,
    ) -> Result<IssuedTicket, ResourceTicketError> {
        validate_token(token)?;
        let mut tickets = self
            .tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let old = tickets
            .get(token)
            .cloned()
            .ok_or(ResourceTicketError::Unknown)?;
        if old.session_id != session_id || old.access != access {
            return Err(ResourceTicketError::Scope);
        }
        if now <= old.expires_at || now > old.expires_at.saturating_add(self.refresh_grace_seconds)
        {
            return Err(ResourceTicketError::Expired);
        }
        tickets.remove(token);
        let mut refreshed = old;
        refreshed.expires_at = now.saturating_add(self.ttl_seconds);
        Ok(mint_locked(&mut tickets, refreshed))
    }
}

fn mint_locked(tickets: &mut HashMap<String, ResourceGrant>, grant: ResourceGrant) -> IssuedTicket {
    let expires_at = grant.expires_at;
    loop {
        let token = uuid::Uuid::new_v4().simple().to_string();
        if tickets.insert(token.clone(), grant.clone()).is_none() {
            return IssuedTicket { token, expires_at };
        }
    }
}

fn validate_token(token: &str) -> Result<(), ResourceTicketError> {
    if token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ResourceTicketError::Unknown)
    }
}
