# oh-my-dream

Local desktop AI creation client built as a Rust workspace with a React/Vite
frontend and a Tauri desktop shell.

The in-app Assistant is an SDK-managed production agent that iteratively plans,
builds, reviews, and repairs the Project's single editable Workflow. Product code
provides trusted tools and lifecycle facts; it does not implement a separate
shot or model/tool loop. See
[`docs/BACKEND_ASSISTANT.md`](docs/BACKEND_ASSISTANT.md) for the frozen MVP
architecture and boundaries.

## Prerequisites

- Rust 1.91 or newer
- Node.js and npm
- Tauri CLI 2.x (`cargo install tauri-cli --locked` if `cargo tauri --version`
  is unavailable)

## Install Dependencies

```bash
cd ui
npm install
cd ..
```

Rust dependencies are resolved by Cargo during build, test, or dev commands.

## Run The App

For the full desktop client, start the frontend in one terminal:

```bash
cd ui
npm run dev -- --host 127.0.0.1 --port 5273
```

Then start the Tauri shell from the repository root in a second terminal:

```bash
cargo tauri dev
```

Tauri connects to the configured dev URL, `http://localhost:5273`, and opens
the desktop window.

For a browser-only UI preview, run:

```bash
cd ui
npm run dev -- --host 127.0.0.1 --port 5273
```

Then open `http://127.0.0.1:5273`. In this mode the frontend uses the mock API
instead of Tauri IPC.

## Verification

Run the full merge gate:

```bash
./scripts/e2e.sh
```

Useful focused checks:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cd ui && npm run typecheck && npm run test
```
