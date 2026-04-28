# ScreenCast Restore Match Rules

OBS may send optional restore match rules to a ScreenCast v6 portal using the
`restore_match_rules` option. The public wire shape is an array of vardicts:

```text
[
  {
    kind = "title_regex",
    pattern = "project-name",
    scope = "same_app" | "any_app",
  },
]
```

Only `kind = "title_regex"` is currently recognized. `pattern` uses Rust
`regex` syntax, is searched case-insensitively anywhere in the window title, and
does not support lookaround or backreferences. Missing OBS scope defaults to
`same_app`; malformed external rules are ignored with a warning.

COSMIC restore tokens are vendor-private. v1 stores outputs and exact toplevel
identifiers, v2 adds app/title rescue metadata, and v3 persists per-window regex
aliases. v1 and v2 tokens continue to deserialize with empty alias lists.

Matching order is:

1. Exact compositor identifier.
2. Regex aliases, when the selected window's rescue mode allows title-style
   matching.
3. Existing app/title fallback behavior.

`same_app` requires the restored window's saved app ID to match the candidate
window. `any_app` may match across app IDs by title.

OBS stores aliases as a multiline string. Each non-empty line is one regex rule:

```text
project-a
same: project-b
same_app: project-c
any: shared-title
any_app: shared-title
```

Unprefixed lines use `same_app`. The picker UI stores the same data as explicit
rows with a regex field and a scope selector.
