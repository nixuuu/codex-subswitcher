# Native UI design system

The application uses GPUI Kit 0.6.0 with the Cargo.lock-resolved Component/Base 0.6.1 APIs. Product styles compose existing library controls; account switching, requests, and persistent state remain owned by `Switcher`.

## Tokens and controls

- `src/palette.rs` owns semantic light/dark colors and synchronizes Component and Base themes. Views read colors through `cx.theme()`.
- `src/ui/tokens.rs` owns typography, geometry, layout widths, and disclosure motion. The system font is retained. Body text is 14 px, captions 12 px, and the page heading 20 px. Small utility spacing follows GPUI's existing 4 px scale; named layout spacing is 8/12/16/20 px.
- Text actions share 32 px height, 12 px horizontal padding, 14 px text, and 6 px corners. Secondary actions share the same muted treatment. **Add account** uses the same geometry with the primary color. Shadows are disabled in both themes.
- Icon actions use a compact 24 px target, a ghost treatment, tooltip, and explicit accessibility name. They do not add a line to the account identity.
- Dialogs use the library's standard 32 px controls, the shared theme radius, and secondary cancel actions. GPUI retains focus, keyboard activation, disabled states, and dialog dismissal.

## Components

`src/ui.rs` composes the page and its single scroll region. `ui/chrome.rs` owns the header, toolbar, operational feedback, empty state, and footer. `ui/account.rs` owns account rows and reset details. `ui/limits.rs` provides the capacity meter. `ui/connection.rs` owns connection instructions and the configuration confirmation. `ui/components.rs` provides shared actions, badges, stacks, panels, and disclosures.

Helpers return native styled elements so callers can refine layout and attach handlers. They do not embed outer margins. Stable account IDs identify rows, buttons, and motion state. Below 900 px, account rows retain the existing two-line layout; the supported window minimum remains 800 px.

## Motion and privacy

Account details and terminal instructions use the same measured height disclosure: 250 ms in either direction with `cubic-bezier(0.22, 1, 0.36, 1)`, adapted from the transitions.dev accordion. GPUI Base owns interpolation and frame scheduling; rapid toggles reverse from the current value. No background animation loop is added. Content padding is inside the reveal, so a closed panel has no residual padding. Actions inside a closing/closed panel are disabled immediately.

The native transition API honors macOS **Reduce Motion** and adopts the target immediately. Email revelation remains a separate explicit dialog, without delayed hiding, animated address changes, or emails in tooltips and notifications. Existing library dialogs and tooltips keep their library behavior; CSS snippets are not applicable to this native renderer.

## Validation

Use `cargo fmt`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked` for permitted local checks. GUI/E2E verification requires separate authorization under the workspace instructions. The visual checklist is light/dark themes, 800/900/940 px window widths, keyboard focus, disabled actions, rapid disclosure reversal, Reduce Motion, and email dialog dismissal. Static checks alone do not verify those rendered states.
