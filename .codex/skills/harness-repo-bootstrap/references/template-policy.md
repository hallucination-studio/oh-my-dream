# Template Policy

Every generated file starts with a managed marker:

`<!-- harness-repo-bootstrap:managed -->`

Update behavior:

- `init`: create missing files and skip existing files unless `--force`
- `update`: create missing files, skip existing unmanaged files, and refresh managed files only when `--refresh-managed` or `--force` is passed

If a file exists without the managed marker, treat it as user-owned unless the human explicitly asks to replace it.
