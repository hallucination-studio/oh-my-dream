from __future__ import annotations

from typing import Any
import unittest

from agents import Runner

from assistant.tests.sdk_runtime_fakes import MultiStepToolModel, build_echo_agent


class NativeAgentLoopTests(unittest.IsolatedAsyncioTestCase):
    async def test_one_runner_call_owns_repeated_tool_turns_and_correction(self) -> None:
        from assistant.sdk_runtime import build_run_config, drain_stream

        expected_steps = [
            "plan-read",
            "first-item",
            "candidate-prepare",
            "validation-failed",
            "candidate-corrected",
            "second-item",
        ]
        invocations: list[str] = []

        async def invoke(_context: Any, arguments: str) -> str:
            invocations.append(arguments)
            return '{"status":"observed"}'

        model = MultiStepToolModel(expected_steps)
        agent, _tool = build_echo_agent(
            name="Native loop agent",
            model=model,
            on_invoke_tool=invoke,
        )

        result = Runner.run_streamed(
            agent,
            "Build the multi-item production plan.",
            max_turns=8,
            run_config=build_run_config(),
        )
        await drain_stream(result)

        self.assertEqual(model.model_calls, len(expected_steps) + 1)
        self.assertEqual(model.requested_steps, expected_steps)
        self.assertEqual(len(invocations), len(expected_steps))
        self.assertEqual(result.final_output, "production turn complete")


if __name__ == "__main__":
    unittest.main()
