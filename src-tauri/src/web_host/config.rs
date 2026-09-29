use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

pub const DEFAULT_HOST_PORT: u16 = crate::DEFAULT_REMOTE_HOST_PORT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub bind: SocketAddr,
    pub assets_dir: PathBuf,
    pub allow_insecure_direct_bind: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_HOST_PORT),
            assets_dir: PathBuf::from("dist"),
            allow_insecure_direct_bind: false,
        }
    }
}

impl HostConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.bind.ip().is_loopback() && !self.allow_insecure_direct_bind {
            return Err("non-loopback bind refused; use Tailscale Serve or the developer-only --allow-insecure-direct-bind override".into());
        }
        if self.assets_dir.as_os_str().is_empty() {
            return Err("production asset directory is required".into());
        }
        Ok(())
    }

    pub fn insecure_bind_warning(&self) -> Option<&'static str> {
        (!self.bind.ip().is_loopback()).then_some(
            "WARNING: INSECURE DIRECT BIND ENABLED; traffic is not protected by the loopback proxy boundary",
        )
    }

    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let mut args = args.into_iter().skip(1);
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--bind" => {
                    config.bind = args
                        .next()
                        .ok_or_else(|| "--bind requires an address".to_string())?
                        .parse()
                        .map_err(|error| format!("invalid --bind address: {error}"))?;
                }
                "--assets-dir" => {
                    config.assets_dir = PathBuf::from(
                        args.next()
                            .ok_or_else(|| "--assets-dir requires a path".to_string())?,
                    );
                }
                "--allow-insecure-direct-bind" => config.allow_insecure_direct_bind = true,
                "--help" | "-h" => return Err("usage: video-creater-host [--bind 127.0.0.1:4777] [--assets-dir dist] [--allow-insecure-direct-bind]".into()),
                unknown => return Err(format!("unknown argument: {unknown}")),
            }
        }
        config.validate()?;
        Ok(config)
    }
}
