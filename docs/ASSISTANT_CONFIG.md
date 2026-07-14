# Assistant Configuration

The Rust app writes `config_root/assistant_config.json`. The packaged Agents
SDK sidecar reads the same file when its composition root needs model settings;
environment variables override file values for local development and tests.

M3 keeps only settings that belong to the sidecar boundary:

| Setting | Environment variable | File key | Default |
|---|---|---|---|
| Enabled | `OMD_ASSISTANT_ENABLED` | `enabled` | `true` |
| OpenAI-compatible base URL | `OMD_ASSISTANT_BASE_URL` | `base_url` | `https://api.openai.com/v1` |
| Model | `OMD_ASSISTANT_MODEL` | `model` | `gpt-5.4` |
| API key | `OMD_ASSISTANT_API_KEY` | `api_key` | none |

API keys are never logged or returned. The public Rust and TypeScript DTOs
expose only `has_key`.

Temperature, tool-iteration limits, prompt suffixes, developer mode, and
product skills are deferred until their owning milestone has a concrete SDK
composition-root contract. They are not accepted by the M3 settings surface.

Example file:

```json
{
  "enabled": true,
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-5.4",
  "api_key": "sk-..."
}
```
