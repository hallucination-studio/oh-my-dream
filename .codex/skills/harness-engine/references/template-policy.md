# Template Policy

Every generated file starts with a managed marker:

`<!-- harness-engine:managed -->`

Init behavior:

- `init`: create missing files for new repositories; when an existing managed harness is detected, refresh managed files and create missing files while preserving unmanaged files

Use `init` as the normal workspace command so creation and reconciliation share one path. Use `--force` only when the human explicitly accepts overwriting.

If a file exists without the managed marker, treat it as user-owned unless the human explicitly asks to replace it.
