# ScreenCast Portal v6 Design

Status: consolidated design note as of 2026-06-04.

This repository prototypes a ScreenCast portal extension for OBS and other
multi-source capture clients. It adds three optional `SelectSources()` options:

| Key | Type | Meaning |
| --- | --- | --- |
| `source_label` | `s` | Human-readable label for this application-level capture request. |
| `restore_policy` | `a{sv}` | Per-reason policy when a provided `restore_token` cannot be honored. |
| `restore_match_rules` | `aa{sv}` | Optional title-regex aliases used by COSMIC rescue for restored windows. |

It also adds one response result:

| Key | Type | Meaning |
| --- | --- | --- |
| `restore_failure` | `a{sv}` | Present for policy-driven skip/error responses and describes the restore failure. |

The ScreenCast interface version is bumped from 5 to 6. Callers should check
`Version >= 6` before sending the new keys. All defaults preserve current
behavior.

## Problem

OBS can create several concurrent portal sessions, such as game capture,
camera, and browser capture. Current portal dialogs look identical, so users
must guess which dialog belongs to which OBS source.

OBS also persists restore tokens. If a token goes stale because a monitor,
window, or permission entry disappears, the current portal behavior falls back
to a picker. In unattended, headless, kiosk, or CI setups, that picker can hang
the workflow indefinitely.

## Current Design

`source_label` is a display hint, not an identifier. Backends may show it in
their picker UI, truncate it, sanitize it, or ignore it. It must be plain text
and must not be interpreted as markup.

`restore_policy` only matters when the caller sends a `restore_token`. Its
type is `a{sv}`:

| Field | Type | Meaning |
| --- | --- | --- |
| `default_action` | `u` | Optional action for recognized reasons without a reason-specific override. Defaults to prompt. |
| `actions` | `a{su}` | Optional map from reason string to action. Unknown reason strings are ignored. |

Actions use these values:

| Value | Name | Behavior |
| --- | --- | --- |
| `0` | prompt | Default. Fall back to the picker as today. |
| `1` | skip | Do not prompt. Return `response=1`, include `restore_failure`, and close the session. |
| `2` | error | Do not prompt. Return `response=2`, include `restore_failure`, and close the session. |

Standardized reason strings are `token_not_found`, `permission_revoked`,
`token_consumed`, and `source_unavailable`. Unknown action values are treated
as absent, falling back to `default_action`, then to prompt.

`restore_failure` has type `a{sv}`:

| Field | Type | Meaning |
| --- | --- | --- |
| `reason` | `s` | One standardized reason string. |
| `action` | `u` | The action that produced the response. |
| `token_invalid` | `b` | `true` when the token should be discarded. False for `source_unavailable`. |

`restore_match_rules` is an array of vardicts. The currently recognized rule is
`kind = "title_regex"` with a string `pattern` and optional `scope` of
`same_app` or `any_app`; missing scope defaults to `same_app`. COSMIC builds
case-insensitive Rust `regex` expressions from those patterns and searches the
candidate window title. The generic portal validates only the outer shape and a
maximum of 64 rules. COSMIC also rejects empty patterns, rejects patterns over
1024 bytes, and compiles regexes with bounded engine memory
(2 MiB size limit and 1 MiB DFA size limit). Unknown kinds, malformed rules,
invalid scopes, and invalid regexes are ignored with warnings rather than
causing the whole selection request to fail.

`xdg-desktop-portal` owns the generic token-resolution policy. It validates
the new options, replaces restore tokens with restore data, and handles
skip/error directly when the token cannot resolve to restore data. It
classifies those failures as `token_not_found`, `permission_revoked`, or
`token_consumed` before applying the policy.

Backends own presentation and backend-specific recovery. XDPH forwards a
sanitized `source_label` to its picker. Cosmic goes further: when the token
resolves to restore data but the target window is not currently available, it
can register a rescue hook, wait for the window to reappear, and then send a
late successful `Start` response. This is backend value-add, not part of the
generic portal contract.

COSMIC rescue stores a per-window rescue mode in the restore token:

| Value | Name | Behavior |
| --- | --- | --- |
| `0` | exact | Only the saved compositor toplevel identifier may match. |
| `1` | app | Try exact identifier, then exact saved app/title, then any current window with the same app ID. |
| `2` | title | Try exact identifier, then exact saved app/title, then `restore_match_rules`. |

The matching order is exact compositor identifier first, exact saved app/title
second, title-regex aliases for title mode third, and app-ID fallback for app
mode last. If external `restore_match_rules` are supplied and no per-window
rules are selected, COSMIC seeds selected windows with those source-level
rules and defaults their rescue mode to title. Without aliases, selected
windows default to app rescue.

`same_app` title-regex rules can auto-rescue only when the candidate app ID
matches the saved app ID. `any_app` rules may identify a cross-application
candidate by title, but COSMIC treats that as requiring re-consent rather than
an unattended auto-restore. This is the confidentiality boundary: an
application-provided regex must not silently redirect an old restore token to a
different application's window. COSMIC gates cross-app alias matches by falling
back to the picker when re-consent is required.

COSMIC restore tokens are vendor-private and currently have three versions.
v1 stores outputs and exact toplevel identifiers and deserializes as exact-only
matching with empty app/title and alias data. v2 adds saved app IDs, titles,
and rescue modes and deserializes with empty alias lists. v3 adds per-window
regex alias patterns and scopes. Unknown token versions are rejected.

OBS sends `source_label` from the OBS source name on portal v6+. It exposes a
single restore-failure action property with default `Skip`, building a
`restore_policy` with that default action so existing OBS restore flows do not
pop unexpected pickers after upgrade. OBS also turns its multiline restore
alias setting into `restore_match_rules` entries when it sends a
`restore_token`: unprefixed lines use `same_app`, and `same:`,
`same_app:`, `any:`, or `any_app:` prefixes select the rule scope.

The Rust `screencast-portal-mock` crate provides isolated D-Bus tests for the
protocol, including version negotiation, backward compatibility, per-reason
policy values, source labels, stale tokens, lifecycle after failure,
concurrency, and rescue scenarios.

The NixOS module exposes per-component toggles under
`services.screencast-portal-rfc.components`. They default to enabled for
backward compatibility, but downstream systems can disable unused patched
packages so only the packages in their actual closure need substituting or
rebuilding.

## Design Decisions

Use option dictionary keys instead of a new D-Bus method. This follows the
portal pattern used by `persist_mode` and `restore_token`, avoids new object
lifecycle rules, and stays backward-compatible.

Put `source_label` on `SelectSources()`, not `CreateSession()`. The label
describes the requested source selection, not the session object itself.

Use a `restore_policy` object instead of the earlier flat
`restore_fail_mode`. This increases implementation work, but lets callers
distinguish permanently invalid tokens from temporarily unavailable restored
sources before the API is standardized.

Use the existing `Request::Response` signal instead of a new signal. Portal
requests already communicate success, cancellation, and errors asynchronously.

Do not add automatic client-side retry. If a token cannot be restored, the
caller can retry explicitly. For Cosmic's window-reappearance case, the portal
pushes a late response instead of requiring OBS to poll.

Treat `source_label` as untrusted plain text at every layer. The generic
portal rejects labels over 256 bytes and passes through strings only.
libportal truncates caller-provided labels to a UTF-8 boundary before sending.
COSMIC strips C0 controls, DEL, and bidi controls, truncates to 256 bytes, and
normalizes empty labels to absent before logging or displaying. XDPH strips
the same control/bidi set before putting the label into the picker
environment. Picker UIs own final display escaping and must not interpret the
label as markup.

## Weaknesses

The contract has two restore-failure layers. `xdg-desktop-portal` handles
"token did not resolve"; Cosmic handles "token resolved, but the restored
window is unavailable". The `source_unavailable` reason codifies that split,
but other backends still need clear guidance on when to use it.

`source_label` still crosses multiple trust boundaries. The authoritative
contract is length/type validation in the generic portal, UTF-8-boundary
truncation in client helpers, and control/bidi stripping plus display escaping
in UI-owning backends and picker processes. Real desktop smoke tests remain
needed because display escaping is toolkit-specific.

`restore_policy` is more expressive than the typical flat portal vardict
option. Reviewers may push back on the nested `actions` map even though it
keeps future extensions out of the top-level namespace.

The response model is signal-only. That matches portal architecture, but
callers that do not listen for `Request::Response` will miss skip/error
outcomes after `SelectSources()` completes normally.

Cosmic rescue matching trades reliability for convenience. Exact matching is
safe but brittle; app-id matching can choose the wrong window; title matching
breaks when titles are dynamic or localized. The 30 second timeout and 64
pending-rescue cap are pragmatic, not protocol-level guarantees. Cross-app
`any_app` title aliases are particularly sensitive because a broad regex could
identify another application's window; COSMIC therefore requires re-consent for
those matches instead of unattended rescue.

The mock test harness validates protocol behavior, but it cannot prove that
the patches still match all upstream UI and compositor edge cases. Patch-apply
checks and real desktop smoke tests remain necessary.

## Alternatives For Next Steps

1. Minimal upstream path: submit the generic `xdg-desktop-portal` XML/C patch,
   libportal API, and OBS client changes first. Keep Cosmic rescue as a
   backend-specific follow-up. This is the smallest review surface.

2. Hardening-first path: finish OBS locale strings, make sanitation rules
   consistent, and document the two restore-failure layers before upstreaming.
   This reduces review churn at the cost of time.

3. Label-only path: upstream `source_label` first and defer
   `restore_policy`. This solves the visible UX problem with very low
   controversy, but leaves unattended stale-token workflows unsolved.

4. Backend-experiment path: ship the generic v6 keys plus Cosmic rescue in a
   Nix overlay and gather real OBS usage data before pushing the rescue design
   upstream. This de-risks matching policy, timeout, and cap choices.

5. Flat-enum fallback path: revert to `restore_fail_mode` if upstream rejects
   the richer object shape. This lowers implementation burden but loses
   reason-specific token cleanup semantics.

## Upstreaming Notes

Patch metadata must follow [docs/patch-conventions.md](docs/patch-conventions.md):
canonical `From`, DCO `Signed-off-by`, SPDX headers on net-new files when the
target project expects them, accurate imperative Subjects, and wider context
for fragile hunks.

Future upstreaming work not performed by this phase:

1. Split the COSMIC patch into a generic backend restore-policy/source-label
   part and a separate rescue UI/token-alias experiment if upstream review asks
   for a smaller surface.
2. Recheck picker markup escaping in the XDPH path after source-label
   sanitization, because control-character stripping does not by itself prove
   toolkit-level plain-text rendering.

## Source Material Folded In

This document consolidates `RFC.md`, `gaps.md`, `CLAUDE.md`,
`claude-code-build-prompt-v4.md`, the patch headers under `patches/`, and the
mock crate README/tests. `docs/restore-match-rules.md` remains the focused
reference for the alias wire shape and OBS shorthand. The older files are
still useful as detailed history, but `DESIGN.md` should be treated as the
concise current design summary.
