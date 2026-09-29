# Remote access

Video Creater's browser host binds to loopback by default. Tailscale Serve supplies the HTTPS
tailnet boundary; do not expose port 4777 directly to a LAN or the public internet.

## Start and verify

The desktop app exposes the installed service under **Settings → Remote access**. Start and Stop are
enabled only when the systemd user unit is installed. The app withholds the tailnet link until the
Serve route and protocol health check both pass; it never displays session credentials or private
host paths.

Build the production frontend, then start the host:

```sh
pnpm build
VIDEO_CREATER_PROJECT_ROOTS=/absolute/path/to/projects \
  pnpm host:dev -- --bind 127.0.0.1:4777 --assets-dir dist
```

The host prints a one-time six-digit pairing code to its local terminal. Keep that terminal private.
In another terminal, opt in to the proxy route:

```sh
tailscale serve --bg --https=443 http://127.0.0.1:4777
cargo run --manifest-path src-tauri/Cargo.toml --features web-host \
  --bin video-creater-host -- --tailscale-status
```

Only use the reported URL when the JSON state is `ready` and `healthVerified` is `true`. The check
requires the current MagicDNS name, an exact Serve proxy to `127.0.0.1:4777`, and the expected
`/healthz` protocol version. It never changes Serve, firewall, or daemon state.

Open the verified HTTPS URL on another device signed into the same tailnet, enter the pairing code,
and name the device. Pairing codes are single-use and expire after five minutes.

## Status and logs

```sh
tailscale status
tailscale serve status
curl --fail --silent https://DEVICE.MAGICDNS.NAME/healthz
systemctl --user status video-creater-host.service
journalctl --user -u video-creater-host.service --since today
```

The status detector distinguishes a missing CLI, stopped daemon, signed-out client, insufficient
permission, missing Serve route, failed health check, and ready state. Logs redact path-bearing RPC
failures and must never contain session cookies, CSRF values, pairing codes beyond the initial local
ready event, provider credentials, or raw proxy identity headers.

## Stop, revoke, and recover

Stop the supervised host immediately (its listener and WebSockets close with the process):

```sh
systemctl --user stop video-creater-host.service
```

Remove only the Video Creater Serve route when remote access is no longer wanted:

```sh
tailscale serve reset
```

Paired-device sessions are stored as hashes in
`$XDG_STATE_HOME/video-creater/remote-devices.json` (or the configured
`VIDEO_CREATER_HOST_DATA_DIR`). Stop the service before replacing that file. To revoke every browser
after a suspected compromise, stop the host, move `remote-devices.json` to a private backup, and
start the host again; all devices must pair again. Never edit individual hashes by hand.

Temporary uploads live under the host state directory and expired completed uploads are removed at
startup. Projects and completed render artifacts remain durable. After a crash, restart the service,
open the project, and use Background tasks as the authoritative job state.

Agent turns run only on the host through the packaged, pinned Codex sidecar. Sign in to Codex on the
host account before starting an agent turn; the browser never receives Codex credentials or direct
app-server access. The sample user service grants the host write access only to its state/project
tree and the host account's existing `.codex` directory.

## Common recovery states

- `daemon_stopped`: start Tailscale using the operating system's normal Tailscale installation.
- `signed_out`: authenticate the machine with `tailscale up` using your normal tailnet policy.
- `unauthorized`: fix local Tailscale/Serve operator permissions; do not run the app as root.
- `serve_not_configured`: run the explicit `tailscale serve --bg` command above.
- `health_unreachable`: confirm the host is listening on loopback, then inspect Serve status and the
  user-service journal. Do not bypass the check with a direct bind.

The sample user unit is [video-creater-host.service](../../packaging/systemd/video-creater-host.service).
`rtk pnpm build:remote-host` creates a self-contained Linux package under
`output/remote-host-package/`. On a clean account, create the private state and configuration
directories before installing the package (replace `RUNTIME_NAME` with the generated directory):

```sh
install -d -m 0755 ~/.local/lib
install -d -m 0700 ~/.local/state/video-creater/projects ~/.config/systemd/user ~/.config/video-creater
cp -a output/remote-host-package/RUNTIME_NAME ~/.local/lib/video-creater-host
cp packaging/systemd/video-creater-host.service ~/.config/systemd/user/
cp packaging/systemd/remote-host.env.example ~/.config/video-creater/remote-host.env
chmod 0600 ~/.config/video-creater/remote-host.env
```

Edit `remote-host.env` for this account's absolute project root, then run:

```sh
systemctl --user daemon-reload
systemctl --user enable --now video-creater-host.service
```

The unit creates `~/.local/state/video-creater` itself as a systemd state directory and treats
`~/.codex` as optional, so the service can start before Codex login. Run `codex login` as the same
user before attempting an agent turn.
