# Native UI design system

The application uses GPUI Kit 0.6.0 with the Cargo.lock-resolved Component/Base 0.6.1 APIs. Product styles compose existing library controls; account switching, requests, and persistent state remain owned by `Switcher`.

## Tokens and controls

- `src/palette.rs` owns semantic light/dark colors and synchronizes Component and Base themes. Views read colors through `cx.theme()`.
- `src/ui/tokens.rs` owns typography, geometry, layout widths, and disclosure motion. The system font is retained. Body text is 14 px, captions 12 px, and the page heading 20 px. Small utility spacing follows GPUI's existing 4 px scale; named layout spacing is 8/12/16/20 px.
- Standard text actions use 32 px height, 12 px horizontal padding, 14 px text, and 6 px corners. Compact panel actions use 24 px height, 8 px horizontal padding, and 12 px text; **Add account** uses this compact geometry with the primary color. Shadows are disabled in both themes.
- Icon actions use a compact 24 px target, a ghost treatment, tooltip, and explicit accessibility name. They do not add a line to the account identity.
- Dialogs use the library's standard 32 px controls, the shared theme radius, and secondary cancel actions. GPUI retains focus, keyboard activation, disabled states, and dialog dismissal.

## Components

`src/ui.rs` composes the compact accounts panel and separate Settings view. `ui/chrome.rs` owns headers, account actions, operational feedback, empty state, and footers. `ui/account.rs` owns account cards and reset details. `ui/limits.rs` provides the capacity meter. `ui/connection.rs` owns connection instructions and the configuration confirmation. `ui/components.rs` provides shared actions, badges, stacks, panels, and disclosures. `src/windows.rs` owns the two window lifecycles and retains a single shared `Switcher` entity even with no windows open.

Helpers return native styled elements so callers can refine layout and attach handlers. They do not embed outer margins. Stable account IDs identify cards, buttons, and motion state. The accounts panel is 460 px wide with a scrollable list between its fixed header/footer. Compact rows use 8 px padding and 4 px content gaps: identity/actions, plan/reset count, then limits with their reset dates inline above the bars. Reset-credit expiration dates stay in the disclosure. Four collapsed accounts with one limit each target roughly the former two-account space. After layout, the panel fits the measured list plus header/footer, including additional limits, warnings, and expanded details. It grows and shrinks with content up to 538 px or the available space below its screen position, whichever is smaller; only the list scrolls beyond that limit. The initial 232 px size is provisional until the first layout. The selected row has a subtle accent surface and border. Settings opens at 740×660 px with a 680×560 px minimum.

Account activation uses a compact 24 px **Activate** button with the default bordered surface, which remains distinct on muted account backgrounds. The active account keeps a disabled **Active** button in the same position. The adjacent chevron opens details and has an explicit tooltip/accessibility label. Other text actions retain the standard 32 px size.

The panel uses a titlebar-free native GPUI `PopUp` window, positioned under the status item's screen rectangle. Tray physical coordinates are converted using its actual AppKit backing scale before clamping to the matching display's visible bounds. Left-click toggles the panel; right-click opens a short native menu. Focus loss and Escape dismiss the panel; dialogs retain their own Escape handling. Closing a panel destroys its transient dialogs, including revealed email addresses, while the shared application model keeps the proxy and background work alive. Settings is observed from the same model, remains a regular window, and exposes setup/diagnostics instead of duplicating them in the panel.

## Motion and privacy

The accounts panel uses macOS `NSVisualEffectView` through GPUI's `Blurred` window background. Its Root is transparent; the product background tint has 52% opacity in light mode and 60% in dark mode to limit interference from desktop content. Account surfaces retain 22% opacity. Text uses near-black/near-white foregrounds and stronger secondary colors at full opacity. Capacity percentages and proxy status use the main foreground; progress bars retain semantic colors, with status also conveyed in words. This is native frosted glass, not a simulated screenshot blur or the Liquid Glass API. Settings retains its opaque surface. The panel's **Add account** action uses the same compact 24 px height as account activation.

Panel closing uses `AnyWindowHandle::update`, so it does not borrow the `Root` entity before clearing its dialogs. Calling the dialog API inside `WindowHandle<Root>::update` would reborrow that entity and panic, including when opening Settings from the panel.

Panel presence fades the whole native window, including its blur layer, over 250 ms on entry and 150 ms on exit with the shared smooth-out curve. `src/panel_effects.rs` accesses the borrowed native window handle on the UI thread without caching native pointers. GPUI Presence owns timing, interruption/reversal, frame scheduling, and Reduce Motion. The window remains mounted until exit completes; mouse input is disabled and email dialogs are cleared as soon as closing begins. Reopening cancels pending removal. There is no sleep-based animation or idle animation loop.

Opening the nonactivating accounts panel does not activate the entire application. Dismissal follows an active-to-inactive window transition; initial inactive notifications during the AppKit focus handoff are ignored. Settings still activates normally as a regular application window.

Account details and terminal instructions use the same measured height disclosure: 250 ms in either direction with `cubic-bezier(0.22, 1, 0.36, 1)`, adapted from the transitions.dev accordion. GPUI Base owns interpolation and frame scheduling; rapid toggles reverse from the current value. No background animation loop is added. Content padding is inside the reveal, so a closed panel has no residual padding. Actions inside a closing/closed panel are disabled immediately.

The native transition API honors macOS **Reduce Motion** and adopts the target immediately. Email revelation remains a separate explicit dialog, without delayed hiding, animated address changes, or emails in tooltips and notifications. Existing library dialogs and tooltips keep their library behavior; CSS snippets are not applicable to this native renderer.

## Validation

Use `cargo fmt`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked` for permitted local checks. Placement unit tests cover screen edges, negative-origin monitors, and short displays. GUI/E2E verification requires separate authorization under the workspace instructions. The visual checklist is light/dark themes, mixed-DPI monitors, left/right tray clicks, outside-click and Escape dismissal, Settings reopen/close, keyboard focus, disabled actions, rapid disclosure reversal, Reduce Motion, and email dialog dismissal. Static checks alone do not verify those rendered states.
