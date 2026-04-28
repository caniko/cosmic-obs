# Manual Restore Alias Checklist

Use this after building and switching an Atlas configuration with the patched
COSMIC portal and patched OBS.

1. In OBS, create a Window Capture (PipeWire) source for a VS Code or Codium
   project A window.
2. Set restore aliases to a regex that should match project B, then close
   project A and open project B.
3. Restart OBS or reload the source and confirm the COSMIC restore rescue
   selects project B without requiring a fresh source.
4. Open the COSMIC picker for a selected window, enter an invalid regex such as
   `(`, and confirm Share is disabled and an inline invalid-regex message is
   shown.
5. Provide malformed external restore data with one invalid rule and one valid
   rule, then confirm restore does not crash and the valid rule can still match.
6. Verify scope behavior:
   `same_app` must not match a title from a different app ID, while `any_app`
   may match the same title across app IDs.
7. Confirm old v1/v2 restore tokens still deserialize and preserve their
   previous exact/app/title behavior with empty alias lists.
8. Confirm multiple selected windows keep separate alias rows after reopening a
   fallback picker prompt.
