# oh-my-dream — AI Assistant Design

> Status: draft v1 · Last updated: 2026-07-08

This document specifies the in-app AI assistant: an agent that can drive every
capability of the client in real time — creating nodes, wiring them, changing
parameters and configuration, running workflows, and managing skills — while
talking to the user in a chat dock.

It is a living document; it is revised as implementation proceeds.

Decisions locked with the product owner (2026-07-08):

- **Packaging**: the Python assistant ships as a **frozen binary** (PyInstaller)
  bundled as a Tauri sidecar. No dependency on the user's Python. (Decision A)
- **Skills**: **declarative-first**. A v1 skill is prompt + a curated set of
  existing capabilities + workflow templates. Code-carrying skills are gated
  behind an explicit developer mode. (Decision B)
- **Chat placement**: a **collapsible right-side dock**, coexisting with the
  canvas. (Decision C)
- **Default model**: `gpt-5.4`. (Decision D)
- **Autonomy**: cost-incurring and destructive actions **always require user
  confirmation**. (Decision E)
- **Transport auth**: the local WebSocket is authenticated (see §4).

---

## 1. Architecture overview

The guiding decision: **the React frontend is the sole executor; Python only
orchestrates.**

The assistant must drive two classes of capability:

- **Backend capabilities** — Tauri commands in `src-tauri` that mutate persisted
  state (projects, assets, provider config, running a workflow).
- **UI capabilities** — actions that live only in React state until saved
  (add a node, connect two ports, change a parameter, select, switch tab).

Their only meeting point is the frontend, which already holds the canvas state
*and* the `invoke()` bridge to Rust. So Python orchestrates and the frontend
executes.

```
┌──────────────────────────────────────────────────────────────┐
│  Tauri desktop client                                          │
│                                                                │
│  React frontend ──invoke()──► Rust backend (commands.rs)       │
│     │   ▲                        │                             │
│     │   │ run UI capabilities    │ run backend capabilities    │
│     │   └────────────────────────┘                             │
│     │            CapabilityExecutor (single dispatch point)    │
│     │                                                          │
│     │  WebSocket (token stream + tool_call / tool_result RPC)  │
│     ▼                                                          │
│  ┌───────────────────────────────┐                            │
│  │ Python assistant sidecar      │  ◄── Rust spawns it,        │
│  │ FastAPI + WebSocket server    │      passes port + token    │
│  │ LangGraph ReAct agent         │      via environment        │
│  │ ChatOpenAI (OpenAI protocol)  │                             │
│  └───────────────────────────────┘                            │
└──────────────────────────────────────────────────────────────┘
          │
          └──► OpenAI-compatible API (configurable base_url)
```

### 1.1 Turn lifecycle

1. The user types in the chat dock. React sends `user_message` over the WS with
   a compact context (project id, canvas snapshot, current selection).
2. Python runs the LangGraph agent loop. `ChatOpenAI` streams tokens, which are
   relayed to the frontend and rendered live.
3. The model emits a tool call. Python sends `tool_call { call_id, capability,
   args }` to the frontend.
4. The frontend's `CapabilityExecutor` dispatches: UI capabilities run locally
   (mutating React state); backend capabilities go through `invoke()` to Rust.
5. The frontend returns `tool_result { call_id, ok, result | error }` to Python,
   which feeds it back to the model and continues the loop.
6. On completion Python sends `message_done`.

"Real time" is native to the WebSocket. "Drives every capability" holds because
the frontend can already reach every capability.

---

## 2. Capability system

A single abstraction, `Capability`, describes one action the assistant may take.
Every capability carries a human-readable description and a per-field parameter
schema, so the model can reason about it without hardcoded knowledge.

```jsonc
{
  "name": "workflow.add_node",
  "description": "Add a node of the given type to the canvas at an optional position.",
  "kind": "backend" | "ui",
  "parameters": {                       // JSON Schema; every field has a description
    "type": "object",
    "properties": {
      "node_type": { "type": "string", "description": "Node type id, e.g. TextToImage" },
      "position":  { "type": "object", "description": "Canvas coordinates; auto-placed if omitted" }
    },
    "required": ["node_type"]
  },
  "returns": { "type": "object", "description": "The created node id" },
  "confirm": false                      // true for cost-incurring / destructive actions
}
```

### 2.1 Two sources, one manifest

- **Backend capabilities** are declared in a new `src-tauri/src/capabilities.rs`
  registry — a description and parameter schema per Tauri command. A new command
  `get_capability_manifest` returns them.
- **UI capabilities** are declared in a TypeScript registry
  (`add_node`, `connect_nodes`, `set_param`, `delete_node`, `select_node`,
  `switch_tab`, `get_canvas_state`, `get_selection`, …), each with its schema.

The frontend merges both into the full manifest and hands it to Python at
handshake time. **Python hardcodes no tools**: it builds LangGraph tools
dynamically from the manifest, where each tool's implementation is "send a WS
RPC to the frontend and await the result." Adding a capability therefore
requires zero changes on the Python side.

### 2.2 "All backend interfaces open by default" — enforced by a contract test

A contract test asserts that **every command registered in `generate_handler!`
has a matching entry in the capability registry** (unless it explicitly opts
out via an allow-list). If you add a Tauri command but forget to expose it —
with a description and parameter descriptions — the test fails. This mirrors the
existing cross-language contract discipline in `src-tauri/tests/contract.rs` and
guarantees the assistant's reach never silently drifts from the real API.

---

## 3. Assistant configuration

Persisted at `config_root/assistant_config.json`, following the exact pattern of
the existing `provider_config.json` (read/write helpers, pretty JSON, config
directory that is never asset-served).

```jsonc
{
  "enabled": true,
  "base_url": "https://api.openai.com/v1",   // any OpenAI-compatible endpoint, incl. local
  "model": "gpt-5.4",
  "api_key": "sk-...",                        // stored in the config dir; never in the repo or logs
  "temperature": 0.3,
  "max_tool_iters": 20,
  "system_prompt_extra": null,
  "skills": { "installed": [], "enabled": [] }
}
```

- **OpenAI protocol only**: the client uses LangChain `ChatOpenAI`. The wire
  protocol is fixed to OpenAI chat-completions; only `base_url` varies, which
  covers OpenAI, compatible gateways, and local servers (Ollama, vLLM, …).
- **Key handling**: the key lives in the config directory. Following the
  existing provider model, a `get_assistant_config` command returns
  `has_key: bool` rather than the raw key, and the settings UI shows only a
  masked value. The key is never logged and never placed on a command line
  (see §5.2).

---

## 4. Real-time transport & authentication

The assistant channel is a WebSocket on `127.0.0.1:<port>`. A local WS is *not*
inherently safe: any process on the machine can attempt to connect, and — more
subtly — a malicious page in the user's browser can attempt Cross-Site WebSocket
Hijacking (CSWSH) by opening `ws://127.0.0.1:<port>`. The channel is therefore
authenticated with four layers of defense.

### 4.1 Defense layers

1. **Loopback only.** The server binds `127.0.0.1`, never `0.0.0.0`. No network
   exposure.
2. **Ephemeral per-launch token.** When Rust spawns the sidecar it generates a
   32-byte CSPRNG token (base64) and passes it **via an environment variable**
   (never argv, never logs). Rust keeps a copy in memory. The token lives for
   the app's lifetime and dies on exit.
3. **First-frame authentication.** The frontend asks Rust for
   `get_assistant_session() → { port, token }` (the token is never hardcoded in
   the frontend). After the socket opens, the client's **first frame must be
   `{ type: "auth", token }`**. The server validates within a 2s timeout;
   failure or timeout ⇒ immediate `close(1008)`. The token is deliberately not
   placed in the URL query (query strings land in access logs) nor in an
   `Authorization` header (the browser WebSocket API cannot set custom headers).
4. **Origin allow-list.** The handshake `Origin` must be the Tauri webview
   origin (`tauri://localhost`, or the dev origin). Anything else is rejected.
   This layer specifically blocks CSWSH: a malicious page carries its own
   origin and cannot obtain the token — two independent barriers.

### 4.2 After authentication

An authenticated connection is a trusted channel; subsequent `tool_call` /
`tool_result` frames are not individually signed (loopback + a validated token
is sufficient for a single-machine client). The session DTOs
(`get_assistant_session`, the `auth` frame) are covered by the cross-language
contract fixtures so the three sides (Rust spawn, TS client, Python server)
cannot drift.

### 4.3 Message protocol

**React → Python**

| Message | Payload |
|---|---|
| `auth` | `{ token }` (first frame) |
| `user_message` | `{ text, context: { project_id, canvas_snapshot, selection } }` |
| `tool_result` | `{ call_id, ok, result \| error }` |
| `confirm_result` | `{ call_id, approved }` |
| `cancel` | `{}` |

**Python → React**

| Message | Payload |
|---|---|
| `auth_ok` / `auth_err` | `{}` / `{ reason }` |
| `token` | `{ delta }` — streamed assistant text |
| `status` | `{ text }` — optional progress ("adding node…") |
| `tool_call` | `{ call_id, capability, args }` |
| `confirm_request` | `{ call_id, capability, args, summary }` |
| `message_done` | `{}` |
| `error` | `{ message }` |

---

## 5. Sidecar lifecycle & packaging

### 5.1 Packaging (Decision A: frozen binary)

- **Dev**: Tauri spawns `python -m assistant`.
- **Prod**: the assistant (Python + LangGraph) is frozen with PyInstaller into a
  standalone executable and bundled as a Tauri `externalBin` sidecar. This keeps
  the "no login / works out of the box / offline-capable" positioning: the user
  never installs Python.

### 5.2 Startup handshake

1. Rust generates the auth token (§4.1) and picks it up for the session.
2. Rust spawns the sidecar, passing the token via environment.
3. The sidecar binds a free loopback port and prints `PORT=<n>` to stdout.
4. Rust reads the port, stores `{ port, token }` in `AppState`, and exposes it to
   the frontend only through `get_assistant_session`.
5. On app exit Rust terminates the sidecar.

No secret ever appears on a command line or in a log line.

---

## 6. LangGraph agent design

- **Shape**: a ReAct-style graph. `system_prompt(manifest summary + active skills
  + session context)` → LLM → tool calls → executed over WS → results folded back
  → loop → final answer.
- **State**: `messages`, `active_skills`, `session_context` (current project /
  selection / canvas summary), `tool_iter` counter (bounded by `max_tool_iters`
  to prevent runaway loops).
- **Streaming**: token-level streaming plus a stream of tool-call events.
- **Human-in-the-loop (Decision E)**: capabilities with `confirm: true` pause the
  graph via LangGraph `interrupt`. The frontend shows a confirmation dialog; a
  `confirm_result` resumes or aborts. Cost-incurring `run_workflow`, deletions,
  and overwrites all take this path.
- **Context strategy**: push one compact canvas snapshot at the start of a turn;
  thereafter the agent pulls detail on demand via read capabilities
  (`get_canvas_state`, `get_selection`) rather than resending the full graph each
  round.

---

## 7. Skills

### 7.1 Package layout

```
skill/
  skill.json         # name / version / description / capabilities[] / requires{}
  prompt.md          # guidance injected into the system prompt when active
  workflows/*.json   # optional: workflow templates the skill can instantiate
  graph.py           # optional, DEVELOPER MODE ONLY: custom LangGraph subgraph/tools
```

### 7.2 Install flow

User installs (from a local file/directory now; a registry/URL later) → the
backend validates and copies the package to `config_root/skills/<name>/` →
records it in `assistant_config.json` → the assistant reloads, merging the
skill's prompt, its curated capability set, and its workflow templates. The
settings dialog gains a skill management panel.

### 7.3 Safety (Decision B: declarative-first)

A v1 skill is **prompt + a curated set of existing capabilities + workflow
templates** and executes no arbitrary code. Code-carrying skills (`graph.py`
running inside the sidecar — an RCE surface) are gated behind an explicit
developer-mode toggle, with a clear warning at install time, exactly as one
would treat an untrusted extension.

---

## 8. Frontend chat dock (Decision C)

A collapsible dock on the right, opened from a third icon in the rail. It
coexists with the canvas (it does not replace the inspector) and matches the
flush, hairline-divided desktop aesthetic established in the UI redesign. It
renders streamed tokens, tool-call activity (as compact inline steps), and
confirmation prompts.

---

## 9. Delivery plan

Using the established split: Claude does frontend / styling / Python / contracts;
Codex does the Rust backend, dispatched by the owner.

| Phase | Scope | Owner |
|---|---|---|
| **M0 Contract foundation** | capability-manifest schema, WS protocol DTOs, `assistant_config` persistence, session DTOs, fixtures | Claude (contracts) + Codex (Rust config) |
| **M1 Capability registry** | Rust `capabilities.rs` + completeness contract test + `get_capability_manifest`; TS UI-capability registry + `CapabilityExecutor` | Codex (Rust) + Claude (TS) |
| **M2 Python sidecar** | FastAPI + WS, LangGraph ReAct, ChatOpenAI, manifest→tools, streaming | Claude (Python) |
| **M3 Wire-up** | Rust spawn + port + token; frontend WS client + chat dock; end-to-end "speak → add node / change config" | Codex (spawn) + Claude (frontend) |
| **M4 Auth & HITL** | first-frame token auth, Origin check, `confirm`→`interrupt`→dialog, cancel, error surfacing, no-secret logging | Claude + Codex |
| **M5 Skills** | package format, install/enable commands, skills directory, loading, management UI | Codex (backend) + Claude (frontend / Python) |
| **M6 Hardening** | full contract + e2e coverage, this document finalized | Claude |

---

## 10. Open questions

- Skill registry / remote distribution (v1 is local-file install only).
- OS keychain for the API key (v1 stores plaintext in the config dir, matching
  the current provider-key model).
- Multi-turn cost accounting surfaced in the dock.
