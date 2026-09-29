use std::collections::BTreeMap;
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    Direct,
    TailscaleServe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyIdentity {
    pub login: String,
    pub display_name: String,
}

pub fn trusted_proxy_identity(
    peer: IpAddr,
    mode: ProxyMode,
    headers: &BTreeMap<String, String>,
) -> Option<ProxyIdentity> {
    if !peer.is_loopback() || mode != ProxyMode::TailscaleServe {
        return None;
    }
    let login = headers.get("tailscale-user-login")?.trim();
    if login.is_empty() || login.len() > 254 {
        return None;
    }
    let display_name = headers
        .get("tailscale-user-name")
        .map_or(login, String::as_str)
        .trim();
    Some(ProxyIdentity {
        login: login.to_owned(),
        display_name: display_name.chars().take(100).collect(),
    })
}
