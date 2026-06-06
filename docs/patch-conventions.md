# Patch Grooming Convention

Patches intended for upstream review must use consistent mail-style metadata:

- `From: "Can H. Tartanoglu" <gpg@rotas.mozmail.com>`
- Include `Signed-off-by: Can H. Tartanoglu <gpg@rotas.mozmail.com>` in the commit message body.
- Add SPDX license identifiers to net-new source files when the upstream project uses SPDX headers or has an established per-file license convention.
- Use `Subject: [PATCH] <component>: <imperative summary>` and make the summary match the actual patch contents. Do not omit shipped public API fields such as `source_label`, `restore_policy`, or `restore_match_rules`.
- Widen minimal-context hunks when a patch is apply-fragile against the intended upstream base.

Phases 01-05 apply this convention to their own `.patch` files. This document defines the convention only; it does not groom patch content itself.
