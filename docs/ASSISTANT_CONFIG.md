# Assistant Configuration

The assistant sidecar (`python -m assistant`) reads configuration from two sources:

1. **File**: `config_root/assistant_config.json` (written by the Rust app)
2. **Environment variables** (higher priority — env overrides file)

This allows flexible configuration for testing, deployment, and local development.

## Configuration Sources & Priority

For each configuration item:
- If the corresponding **env variable is set**, use it
- Else if the **file value exists**, use it  
- Else use the **default value**

Example:
```bash
# File has: model = "gpt-4"
# Env has:  OMD_ASSISTANT_MODEL="gpt-5.4"
# Result:   model = "gpt-5.4" (env wins)
```

## Environment Variables

All env vars are prefixed with `OMD_ASSISTANT_`. Here's the full reference:

| Env Variable | Type | Default | File Key | Description |
|---|---|---|---|---|
| `OMD_ASSISTANT_ENABLED` | bool | `true` | `enabled` | Enable/disable the assistant |
| `OMD_ASSISTANT_BASE_URL` | string | `https://api.openai.com/v1` | `base_url` | OpenAI-compatible API endpoint |
| `OMD_ASSISTANT_MODEL` | string | `gpt-5.4` | `model` | LLM model to use |
| `OMD_ASSISTANT_API_KEY` | string | (none) | `api_key` | API key for the LLM provider (never logged) |
| `OMD_ASSISTANT_TEMPERATURE` | float | `0.3` | `temperature` | Sampling temperature (0–2) |
| `OMD_ASSISTANT_MAX_TOOL_ITERS` | integer | `20` | `max_tool_iters` | Max tool-call iterations per run |
| `OMD_ASSISTANT_SYSTEM_PROMPT_EXTRA` | string | (none) | `system_prompt_extra` | Extra system prompt to append |
| `OMD_ASSISTANT_DEVELOPER_MODE` | bool | `false` | `developer_mode` | Allow code-based skills |
| `OMD_ASSISTANT_ENABLED_SKILLS` | string (comma-separated) | (empty) | `skills.enabled` | Comma-separated list of enabled skill names (spaces trimmed) |

## Type Conversions

- **bool**: recognized values are `true`, `1`, `yes` (case-insensitive). Anything else is `false`.
- **string**: used as-is
- **float**: parsed with Python `float()`
- **integer**: parsed with Python `int()`
- **string list**: split by `,`, each item trimmed. Example: `skill-a, skill-b, skill-c` → `["skill-a", "skill-b", "skill-c"]`

## Examples

### Example 1: Override model for testing
```bash
OMD_ASSISTANT_MODEL=gpt-4 python -m assistant
```

### Example 2: Disable assistant entirely
```bash
OMD_ASSISTANT_ENABLED=false python -m assistant
```

### Example 3: Use a custom API endpoint
```bash
OMD_ASSISTANT_BASE_URL=https://my-proxy.example.com/v1 \
OMD_ASSISTANT_API_KEY=my-key \
python -m assistant
```

### Example 4: Enable specific skills
```bash
OMD_ASSISTANT_ENABLED_SKILLS="portrait-helper, cinematic-shots" \
OMD_ASSISTANT_DEVELOPER_MODE=true \
python -m assistant
```

### Example 5: Override everything (useful for CI/testing)
```bash
OMD_ASSISTANT_ENABLED=true \
OMD_ASSISTANT_BASE_URL=https://api.openai.com/v1 \
OMD_ASSISTANT_MODEL=gpt-5.4 \
OMD_ASSISTANT_API_KEY=sk-... \
OMD_ASSISTANT_TEMPERATURE=0.3 \
OMD_ASSISTANT_MAX_TOOL_ITERS=20 \
OMD_ASSISTANT_DEVELOPER_MODE=false \
OMD_ASSISTANT_ENABLED_SKILLS="" \
python -m assistant
```

## File Format (assistant_config.json)

For reference, the JSON file structure is:

```json
{
  "enabled": true,
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-5.4",
  "api_key": "sk-...",
  "temperature": 0.3,
  "max_tool_iters": 20,
  "system_prompt_extra": null,
  "developer_mode": false,
  "skills": {
    "enabled": ["portrait-helper", "cinematic-shots"]
  }
}
```

## Notes

- **API keys are never logged or returned in summaries** (public_summary() masks them as `has_key: true`).
- Environment variables take precedence, so you can safely override any file setting.
- The file is written by the Rust backend; the Python sidecar reads it (read-only).
- Comma-separated lists are space-tolerant: `"a, b, c"` and `"a,b,c"` both work.
