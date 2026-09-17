# Codex Sub Switcher

A native macOS app built with Rust and GPUI Kit for switching subscription accounts used by Codex CLI. A local proxy selects the account for each new request. Switching does not restart the CLI or interrupt a response already in progress.

The app interface is in English, including menus, dialogs, notifications, and error messages. Dialogs provided by macOS or your browser may follow the system language.

Account email addresses are hidden by default. Rows, account details, reset confirmations, menu bar text and tooltips, and system notifications use a stable label such as **Account a1b2c3d4**. Click the eye icon next to an account label (tooltip: **Show email**) to reveal its address in a dialog; **Hide email** or closing the dialog hides it again. Revealing an address does not change the menu bar or notifications, and is not remembered after restarting. Browser sign-in pages are outside this protection.

The main view is a compact accounts panel opened from the macOS menu bar. Connection instructions and diagnostics live in a separate **Settings** window.

The panel has a native frosted-glass background and fades in/out when toggled from the menu bar (250/150 ms). macOS **Reduce Motion** makes the transition immediate. Closing clears any revealed email immediately, before the fade finishes.

## Getting started

Requires macOS, Rust/Cargo, and Codex CLI. The integration was checked with **Codex CLI 0.154.0**.

```sh
scripts/bundle.sh
open 'dist/Codex Sub Switcher.app'
```

For development, use `cargo run --locked`. The app bundle is signed ad hoc locally; it is not a notarized distribution release.

1. Click **Add account…** and choose **Sign in on this computer…** for the standard browser flow, or **Sign in on another computer…** to get a device sign-in link and one-time code. Use **Copy link and code**, send both to your other computer, open the link there, sign in to the new account, and enter the code. Keep the switcher running: the account is added here automatically. The code expires after 15 minutes; **Cancel sign-in** stops waiting. Device sign-in requires an up-to-date Codex CLI and device code login enabled in [ChatGPT security settings or workspace permissions](https://developers.openai.com/codex/auth/#preferred-device-code-authentication-beta). Repeat for another account. Each sign-in uses its own working directory and does not sign out your regular CLI. With no saved accounts, the panel also links directly to the import section in Settings.
2. Click **Activate** next to the account you want to use. The selected account shows **Active**.
3. Connect your terminal using one of the options below.
4. When a limit is exhausted, click **Activate** next to another account. The next request will use it. The app does not switch accounts or retry generation automatically after a limit error.

The app must remain running while you use the proxy. Left-click the menu-bar percentage to toggle the accounts panel. It opens below the status item, stays within the usable display bounds, and dismisses when focus leaves it or you press **Escape** (an open dialog handles Escape first). Accounts scroll within the panel; the header and refresh status remain visible. Closing the panel preserves the proxy, account operations, and limit refreshes. The menu bar shows the active account's **remaining** capacity: **91%** for a single window, or **5h: 40% · 7d: 80%** for both. Values update after each refresh (automatically every minute) and when switching accounts. The last result remains visible during a refresh; no active account, missing data, or a refresh error displays **—%**. All displayed percentages are whole numbers.

The gear button or **⌘,** opens **Settings** with connection instructions, **Import from CLI…**, notification testing, an explicit Light/Dark Appearance choice, and request counters. The panel footer's **Connect terminal…** action opens the same window at expanded connection instructions. Settings is a separate, resizable application window with a Dock icon. Closing Settings returns to menu-bar operation. Right-click the menu-bar percentage for **Accounts**, **Settings…**, and **Quit Codex Sub Switcher**. Only **Quit** or **⌘Q** stops the proxy. If creating the menu-bar item fails, Settings opens as the regular app window and offers **Open accounts…**; closing the last window then quits.

### Connect without changing configuration

Use **Connect terminal…** in the panel footer, or choose **Show instructions** in Settings. Click **Copy command** and paste it into a terminal in your project directory. The `codex-switch` launcher forwards arguments to the real CLI. You can add `resume`, `resume --last`, or other standard Codex arguments.

The regular `codex` command continues using its existing configuration. A process started outside the proxy needs to be stopped and resumed through the launcher once; subsequent account switches do not require a restart.

### Optional `config.toml` integration

Click **Enable in config.toml…** and confirm the change shown in the dialog. The app:

- Creates a private configuration backup.
- Changes the default `model_provider` to `subscription_switcher`.
- Adds a Responses provider pointing to `127.0.0.1`.
- Configures a helper command to supply the local proxy key, without OAuth tokens in `config.toml`.
- Preserves comments and other settings.

Afterward, **new `codex` sessions started from any directory** use the proxy unless project configuration, the selected profile, or a `-c` argument overrides the provider. The change applies to the `CODEX_HOME` shown in the dialog.

**Restore config.toml…** restores the previous provider selection and removes the app's proxy section while preserving later unrelated edits. If the proxy section has been changed manually, the app refuses to overwrite it and retains the backup. Backups remain in the app's data directory. Restore the configuration before moving or removing the app, because it references the executable's absolute path.

## Behavior and limitations

The native interface shares [design tokens and components](docs/design-system.md). Text actions use consistent sizing and styling; account details and connection instructions expand and collapse with a short transition that respects macOS Reduce Motion.

- Account selection applies to every terminal using this proxy. A request started before a switch remains assigned to its original account, including during token refresh.
- The proxy supports the model list, Responses over HTTP/SSE, and the compaction endpoint. WebSocket is disabled in the provider configuration. `previous_response_id` continuation identifiers are rejected; the CLI must send conversation history.
- Conversations, tools, files, and permissions remain with the CLI. The proxy does not modify history or saved sessions. A complete long-conversation scenario with compaction and switching between **two real accounts** still needs verification.
- Each account row shows the plan saved at sign-in and the remaining capacity of the windows reported by the Codex limits service, such as 5h and weekly. **% left** and bar fill represent remaining capacity: 100% is full and 0% is empty. Bars and values are green above 10% and red at 10% or below. Available windows are never inferred from the plan.
- Reset times use local dates and times. Passing a reset deadline does not restore capacity locally. Data refreshes at startup, after account operations, every minute after the previous refresh completes, and through **Refresh**. Errors preserve the last result with an outdated-data warning. Missing windows do not imply an unlimited subscription.
- Footer counters only cover generation traffic through this proxy. **Proxy running** means that the local service is available; it does not detect whether a terminal uses it. CLI `/status` is not an authoritative source for the selected proxy account.
- Revoked or invalid credentials require adding the account again. Token refresh is automatic, persists rotated refresh tokens, and retries once after HTTP 401 before response streaming begins.
- **Import from CLI…** supports `file` credential storage. This migrates an existing sign-in: do not use copies of it simultaneously in old CLI processes and the proxy, because refreshing may rotate their shared refresh token. Use **Add account…** for independent simultaneous sessions. `keyring`, `auto`, and `ephemeral` stores are not imported.
- The proxy handles model-provider traffic. It does not replace the identity used by other Codex services, such as cloud tasks or connectors.

## Limit reset notifications

After a successful refresh, the app compares each account's limits with its previous successful reading. When a window's remaining capacity returns from below 100% to exactly 100%, it sends a system notification naming the account and restored limits. This also applies to inactive accounts and while the panel is closed. Clicking the notification opens the accounts panel.

The API still reports usage as `used_percent`, so detecting a full reset means comparing previous usage greater than zero with new usage equal to zero. Rounding the displayed remaining capacity to 100% does not trigger a notification.

The first reading after launch establishes a baseline. Repeated readings of 100% remaining do not repeat the notification; using some capacity and then restoring it fully can trigger another. A failed reading, missing window, or elapsed reset deadline does not trigger a notification. Manual resets initiated in the app, including those awaiting confirmation, are excluded. The comparison detects an observed reset; it does not distinguish scheduled resets from additional capacity granted by the service.

Notifications require running **Codex Sub Switcher.app** and granting macOS notification permission. The app requests permission at startup, without waiting for a limit reset. Notifications detected while the permission request is pending are sent only after permission is granted, following [Apple's authorization guidance](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications). Denial or an authorization error discards pending notifications.

Permission status appears below the refresh information. **Test notification** checks permission again and sends an example notification without fetching or resetting limits. macOS remembers previous decisions, so another launch or test may not show a prompt. Change permission in **System Settings → Notifications → Codex Sub Switcher**, then use the test button. **Permission granted** reports authorization, not confirmation that a banner was displayed; presentation also depends on notification settings and Focus mode. When using `cargo run`, the app explains that notifications require an `.app` bundle.

## Manual limit resets

Each compact account row shows the plan, number of reset credits, and reported limits. Without expanding the account, a weekly bar shows day ticks and weekday labels, the current even-usage budget marker, expected remaining capacity, and deviation in percentage points. **Faster than weekly budget**, **On pace with weekly budget**, or **Slower than weekly budget** compares consumption with an even weekly allowance using a one-percentage-point neutral band, not a forecast of recent request speed. The chevron next to **Activate** / **Active** expands account details, including the budget explanation, expiration dates, disabled reasons, and reset action. Credits are ordered by their exact expiration time, with undated credits last. **Next to use** identifies the earliest-expiring eligible `codex_rate_limits` credit; expired, redeemed, and unknown types are excluded.

**Use reset…** opens a destructive confirmation naming the account and the selected credit's expiration date. **Remove account…** also names the private account label and explains that removal does not cancel the subscription. The app sends the specific `credit_id` from the latest reading rather than relying on server ordering. If the credit expires or the selection changes while the dialog is open, confirmation is required again. Credits are never consumed or purchased automatically.

After a network error, **Retry reset…** uses the same credit ID and operation ID. The request is retained across launches until a recognized response arrives. New resets are blocked when expiration dates cannot be fetched, readings are outdated, or no eligible credit is available; the detail area names the matching reason and points to Refresh when it can recover the data. Limits are fetched again after the operation; the UI does not restore percentages on its own.

[Expiration details](docs/reset-expirations.png) · [Confirmation dialog](docs/reset-confirmation.png) · [Earlier validation](docs/tray-resets-validation.md)

## Data and security

Data is stored in `~/Library/Application Support/Codex Sub Switcher` by default. Profiles contain OAuth tokens in JSON files, like the CLI's file credential store; **they are not encrypted using Keychain**. Directories use `0700` permissions; credential files and configuration backups use `0600`. Writes use a temporary file, `fsync`, and atomic replacement. Account data is not stored in this repository.

The proxy listens only on IPv4 loopback and requires a random 256-bit local key. It rejects browser requests carrying an Origin header. The key and port persist across launches. Model-provider authentication obtains the key through `auth.command`, not through environment variables passed to CLI tools. Subscription tokens are sent only to fixed OpenAI/ChatGPT endpoints. Prompts and credentials are not logged.

Environment overrides:

- `CODEX_HOME`: CLI configuration and session directory.
- `CODEX_SWITCHER_HOME`: separate switcher data directory.
- `CODEX_SWITCHER_CODEX`: absolute path to the installed CLI.

## Checks

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

An optional test using an installed CLI uses only synthetic credentials and a local server:

```sh
cargo test --locked installed_codex_accepts -- --ignored
```

The separate `live_subscription_smoke` test is ignored by default. It requires explicitly setting `CODEX_SWITCHER_LIVE_AUTH` to a local credentials file and building `target/debug/codex-sub-switcher`. It makes one small request using a real subscription and does not refresh or save the source credentials.

For GUI screenshots, generate six synthetic accounts with `python3 scripts/demo-data.py /tmp/switcher-demo/store` and use separate `CODEX_SWITCHER_HOME` and `CODEX_HOME` directories. The debug-only `--demo-usage` argument (or `CODEX_SWITCHER_DEMO=1`) displays synthetic limits without calling the usage endpoint, consuming reset credits, or requesting system notification permission. The app labels this as demo mode and disables the notification test button. Never send synthetic credentials to the real service.

Earlier validation reports: [limits](docs/usage-validation.md), [tray and manual resets](docs/tray-resets-validation.md), and [compact layout](docs/design-validation.md). These reports describe their dated verification runs, not every subsequent change.

## Screenshots

These screenshots were captured on **September 12, 2026**, using the earlier standalone-window layout and six synthetic accounts. They predate the menu-bar panel redesign; new panel/settings screenshots have not yet been captured. See [capture details](docs/screenshots.md) for the earlier scope.

- [Light theme](docs/compact-light.png)
- [Dark theme](docs/compact-dark.png)
- [Expanded account details](docs/compact-details-dark.png)
- [Narrow window](docs/compact-narrow.png)
- [Reset expiration dates](docs/reset-expirations.png)
- [Reset confirmation](docs/reset-confirmation.png)
- [Terminal connection instructions — light](docs/ui-light.png)
- [Terminal connection instructions — dark](docs/ui-dark.png)

## Sources and compatibility

- [Codex authentication](https://learn.chatgpt.com/docs/auth): subscriptions, credential storage, and refresh.
- [Codex configuration](https://learn.chatgpt.com/docs/config-file/config-reference): model providers and client configuration.
- [Codex 0.154.0 source](https://github.com/openai/codex/tree/36eab01061df3cde5f95ec20a526777b430091ba): `model_providers.auth`, transport, and token lifecycle. This integration uses the Codex subscription backend, which can change independently of the public API.

The `gpui-kit` facade is pinned to 0.6.0. `Cargo.lock` resolves `gpui-component`, `gpui-base`, and `gpui-kit-assets` to 0.6.1, and `gpui-pre` to 0.3.4. APIs were checked against these sources.
