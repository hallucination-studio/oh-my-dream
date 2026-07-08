"""LangGraph-native assistant agent.

This module builds a ReAct agent that exposes capabilities as tools. Each tool's
execution is paused via interrupt() — the frontend executes the capability and
resumes with the result. This keeps the agent graph pure Python orchestration
without baking in the actual capability implementations (which live in the Rust
backend and browser).

The imports of langgraph/langchain are optional and lazy, so tests pass without them.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import logging
from typing import Any

from .config import AssistantConfig
from .skills import Skill

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class AgentContext:
    config: AssistantConfig
    manifest: dict[str, Any]
    skills: list[Skill]


def build_system_prompt(context: AgentContext) -> str:
    """Assemble the system prompt from config + enabled skills."""
    capability_names = [
        capability.get("name", "")
        for capability in context.manifest.get("capabilities", [])
        if isinstance(capability, dict)
    ]
    parts = [
        "You are the in-app oh-my-dream assistant.",
        f"Model: {context.config.model}",
        "Available capabilities:",
        "\n".join(f"- {name}" for name in capability_names if name),
    ]
    for skill in context.skills:
        parts.append(f"Skill {skill.name}:\n{skill.prompt}")
    if context.config.system_prompt_extra:
        parts.append(context.config.system_prompt_extra)
    return "\n\n".join(parts)


def build_graph(
    context: AgentContext,
) -> Any:
    """Build a LangGraph ReAct agent whose tools pause via interrupt().

    Each tool (capability) calls interrupt() with the tool description, pausing
    the graph so the frontend can execute it and resume with the result.

    Returns a compiled LangGraph that can be streamed with .astream(stream_mode=[...]).
    Must be called *within* an environment where langgraph and langchain_openai are
    installed. Raises ImportError if they are missing.
    """
    try:
        from langchain_openai import ChatOpenAI
        from langgraph.prebuilt import create_react_agent
        from langchain_core.tools import tool as langchain_tool
    except ImportError as error:
        raise ImportError(
            "LangGraph and langchain_openai are required to run the agent. "
            "Install them from assistant/requirements.txt."
        ) from error

    # The assistant's system prompt assembles all capabilities + enabled skill prompts.
    system_prompt = build_system_prompt(context)

    # Build tools from the capability manifest. Each tool's implementation pauses
    # via interrupt(), allowing the frontend to execute the actual capability.
    tools = []
    for capability in context.manifest.get("capabilities", []):
        name = capability.get("name", "")
        description = capability.get("description", "")

        if not name or not description:
            continue

        # Create a tool that interrupts with the capability request.
        def make_tool(cap_name: str, cap_desc: str) -> Any:
            @langchain_tool(name=cap_name, description=cap_desc)
            def capability_tool(**kwargs: Any) -> str:
                # Interrupt pauses the graph, surfacing the tool call to the frontend.
                # The frontend executes the capability and sends the result back via
                # tool_result, which resumes here.
                from langgraph.types import interrupt

                result = interrupt(
                    {
                        "capability": cap_name,
                        "args": kwargs,
                        "description": cap_desc,
                    }
                )
                return json.dumps(result)

            return capability_tool

        tool = make_tool(name, description)
        tools.append(tool)

    # Build the LLM client from the assistant config (OpenAI-compatible).
    llm = ChatOpenAI(
        base_url=context.config.base_url,
        api_key=context.config.api_key,
        model=context.config.model,
        temperature=context.config.temperature,
    )

    # Create the ReAct agent with the LLM and tools.
    agent = create_react_agent(
        model=llm,
        tools=tools,
        state_modifier=system_prompt,
        max_iterations=context.config.max_tool_iters,
    )

    return agent


async def stream_agent_run(
    graph: Any,
    messages: list[dict[str, str]],
    stream_mode: list[str] | None = None,
) -> Any:
    """Stream an agent run via astream.

    Yields (event_type, data) tuples for each message/update/task event.
    The frontend listens to these and surfaces them to the dock.

    Args:
        graph: Compiled LangGraph agent (from build_graph).
        messages: Chat messages to feed the agent.
        stream_mode: List of modes to stream (default: ["messages", "updates"]).
                     "messages" yields LLM token deltas;
                     "updates" yields node name + state diffs.

    Yields:
        (event_type, event_data) tuples for streaming UI updates.
    """
    if stream_mode is None:
        stream_mode = ["messages", "updates"]

    input_state = {"messages": messages}
    try:
        async for event in graph.astream(input_state, stream_mode=stream_mode):
            yield event
    except Exception as error:
        logger.error("Agent stream error: %s", error, exc_info=True)
        raise
