# SVOI integration

In Settings → Integrations → VPN select Happ, SVOI or Disabled. Existing profiles default to Happ. Full settings export/import transfers the provider selection; authorization stays on the PC in SVOI.

Start this Island build, choose SVOI, then in SVOI Settings → Integrations enable «Разрешить управление из Night Island». Check the executable path shown there. Replacing the Island executable requires a new grant.

The home VPN widget supports all existing sizes: one cell provides status and quick action; two cells show the active server and mode; four cells add favorites. More favorites are paged with the More button. Latency is shown only after a successful real connection check. Closed SVOI, missing permission and a disconnected connection have separate states/actions.

Confirmed server changes use the existing shield and temporary notice. Repeated snapshots do not repeat the animation or change the saved music/clock/weather layout. Errors keep the actual active server. UI work and commands run on separate threads; requests use the local restricted SVOI pipe and never receive its private HTTP capability, subscriptions or configuration files.

Regression checks: `cargo test --locked --bin night-island`. The SVOI repository also contains `tests/night-island-windows.py` for two-binary Windows tests with a real local VLESS/Xray fixture. A read-only diagnostic is `night-island.exe --svoi-integration-check state`; it uses the same authorized pipe client.

Verified: settings/profile transfer, default Happ provider, state/action layout bounds, one-shot notice decisions, real two-binary connection/switch/failure/restart/revocation and rejection of another executable. External VPN subscriptions, a live Happ connection and full native-window clicks remain manual checks. Computer Use could not initialize (`failed to write kernel assets`). Voice control is outside this change.
