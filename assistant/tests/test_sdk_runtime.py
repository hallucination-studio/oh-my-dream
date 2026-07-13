from __future__ import annotations

import importlib
import importlib.util
import inspect
import json
from pathlib import Path
import tempfile
from typing import Any
import unittest

from agents import (
    Agent,
    FunctionTool,
    Model,
    Runner,
    UserError,
)
from agents.run_config import ModelInputData
from tests.sdk_runtime_fakes import (
    DeterministicToolModel,
    NonMappingContext,
    RecordingFinalModel,
    build_echo_agent,
)


ASSISTANT_ROOT = Path(__file__).resolve().parents[1]


class SdkDependencyTests(unittest.TestCase):
    def test_openai_agents_is_pinned_to_characterized_version(self) -> None:
        requirements = (ASSISTANT_ROOT / "requirements.txt").read_text(encoding="utf-8")

        self.assertIn("openai-agents==0.18.1", requirements.splitlines())


class SdkRuntimeModuleTests(unittest.TestCase):
    def test_sdk_runtime_module_exists(self) -> None:
        self.assertIsNotNone(importlib.util.find_spec("sdk_runtime"))

    def test_build_run_config_disables_sdk_tracing(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        build_run_config = getattr(sdk_runtime, "build_run_config", None)

        self.assertTrue(callable(build_run_config))
        self.assertTrue(build_run_config().tracing_disabled)

    def test_build_run_config_accepts_sdk_input_callbacks(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        parameters = inspect.signature(sdk_runtime.build_run_config).parameters

        self.assertIn("session_input_callback", parameters)
        self.assertIn("call_model_input_filter", parameters)

    def test_fake_models_match_public_model_method_signatures(self) -> None:
        for fake_model in (DeterministicToolModel, RecordingFinalModel):
            for method_name in ("get_response", "stream_response"):
                public_signature = inspect.signature(getattr(Model, method_name))
                fake_signature = inspect.signature(getattr(fake_model, method_name))

                self.assertEqual(
                    tuple(fake_signature.parameters),
                    tuple(public_signature.parameters),
                )
                for parameter_name, public_parameter in public_signature.parameters.items():
                    fake_parameter = fake_signature.parameters[parameter_name]
                    self.assertEqual(fake_parameter.kind, public_parameter.kind)
                    self.assertEqual(fake_parameter.default, public_parameter.default)
                    self.assertEqual(fake_parameter.annotation, public_parameter.annotation)


class StreamedRunnerTests(unittest.IsolatedAsyncioTestCase):
    async def test_streamed_runner_executes_one_function_tool_and_drains_events(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        drain_stream = getattr(sdk_runtime, "drain_stream", None)
        invocations: list[str] = []

        async def invoke(_context: Any, arguments: str) -> str:
            invocations.append(arguments)
            return "echoed"

        agent, _tool = build_echo_agent(
            name="Characterization agent",
            model=DeterministicToolModel(),
            on_invoke_tool=invoke,
        )

        self.assertTrue(callable(drain_stream))
        result = Runner.run_streamed(agent, "Use the echo tool.", run_config=sdk_runtime.build_run_config())
        events = await drain_stream(result)

        self.assertGreater(len(events), 0)
        self.assertEqual(invocations, ['{"value":"canvas"}'])
        self.assertEqual(result.final_output, "tool completed")


class FileSessionTests(unittest.IsolatedAsyncioTestCase):
    async def test_file_session_reopens_durable_history(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        build_file_session = getattr(sdk_runtime, "build_file_session", None)

        self.assertTrue(callable(build_file_session))
        with tempfile.TemporaryDirectory() as directory:
            database = Path(directory) / "assistant-session.sqlite3"
            first = build_file_session("project-1", database)
            await first.add_items([{"role": "user", "content": "durable input"}])
            first.close()

            reopened = build_file_session("project-1", database)
            history = await reopened.get_items()
            reopened.close()

        self.assertEqual(history, [{"role": "user", "content": "durable input"}])

    async def test_session_and_model_input_callbacks_shape_the_model_call(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        callback_history: list[list[Any]] = []
        filter_inputs: list[list[Any]] = []

        async def merge_session(history: list[Any], new_input: list[Any]) -> list[Any]:
            callback_history.append(history)
            return history + new_input

        def filter_model_input(data: Any) -> ModelInputData:
            filter_inputs.append(data.model_data.input)
            marker = {"role": "developer", "content": "filtered context"}
            return ModelInputData(
                input=[marker, *data.model_data.input],
                instructions=data.model_data.instructions,
            )

        with tempfile.TemporaryDirectory() as directory:
            session = sdk_runtime.build_file_session(
                "project-2", Path(directory) / "assistant-session.sqlite3"
            )
            await session.add_items([{"role": "user", "content": "stored history"}])
            model = RecordingFinalModel()
            agent = Agent(name="Input shaping agent", model=model)
            result = Runner.run_streamed(
                agent,
                "new input",
                session=session,
                run_config=sdk_runtime.build_run_config(
                    session_input_callback=merge_session,
                    call_model_input_filter=filter_model_input,
                ),
            )
            await sdk_runtime.drain_stream(result)
            session.close()

        self.assertTrue(callback_history)
        self.assertTrue(filter_inputs)
        self.assertEqual(callback_history[0][0]["content"], "stored history")
        self.assertEqual(filter_inputs[0][0]["content"], "stored history")
        self.assertEqual(model.inputs[0][0]["content"], "filtered context")


class ApprovalStateTests(unittest.IsolatedAsyncioTestCase):
    async def test_static_approval_restores_strict_state_and_resumes_same_session(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        restore_run_state = getattr(sdk_runtime, "restore_run_state", None)
        invocations: list[str] = []
        invocation_projects: list[str] = []

        async def invoke(context: Any, arguments: str) -> str:
            invocations.append(arguments)
            invocation_projects.append(context.context["project_id"])
            return "approved"

        def build_agent() -> tuple[Agent[Any], DeterministicToolModel, FunctionTool]:
            model = DeterministicToolModel()
            agent, tool = build_echo_agent(
                name="Approval agent",
                model=model,
                on_invoke_tool=invoke,
                needs_approval=True,
            )
            return agent, model, tool

        initial_agent, initial_model, initial_tool = build_agent()

        self.assertTrue(callable(restore_run_state))
        self.assertIs(initial_tool.needs_approval, True)
        with tempfile.TemporaryDirectory() as directory:
            session = sdk_runtime.build_file_session(
                "project-approval", Path(directory) / "assistant-session.sqlite3"
            )
            paused = Runner.run_streamed(
                initial_agent,
                "Use the approved tool.",
                context={"project_id": "project-approval"},
                session=session,
                run_config=sdk_runtime.build_run_config(),
            )
            await sdk_runtime.drain_stream(paused)

            self.assertEqual(invocations, [])
            self.assertEqual(len(paused.interruptions), 1)
            state_json = json.loads(
                json.dumps(paused.to_state().to_json(strict_context=True))
            )
            restored_agent, restored_model, restored_tool = build_agent()
            self.assertIsNot(restored_agent, initial_agent)
            self.assertIsNot(restored_model, initial_model)
            self.assertIsNot(restored_tool, initial_tool)
            restored = await restore_run_state(
                restored_agent,
                state_json,
                context_override={"project_id": "project-restored"},
            )
            interruptions = restored.get_interruptions()
            self.assertEqual(len(interruptions), 1)
            restored.approve(interruptions[0])

            resumed = Runner.run_streamed(
                restored_agent,
                restored,
                session=session,
                run_config=sdk_runtime.build_run_config(),
            )
            await sdk_runtime.drain_stream(resumed)
            session.close()

        self.assertEqual(invocations, ['{"value":"canvas"}'])
        self.assertEqual(invocation_projects, ["project-restored"])
        self.assertEqual(resumed.final_output, "tool completed")

    async def test_strict_restore_rejects_non_mapping_context_without_override(self) -> None:
        sdk_runtime = importlib.import_module("sdk_runtime")
        context_parameter = inspect.signature(
            sdk_runtime.restore_run_state
        ).parameters["context_override"]

        self.assertIsNone(context_parameter.default)

        async def invoke(_context: Any, _arguments: str) -> str:
            return "not executed"

        def build_agent() -> Agent[NonMappingContext]:
            agent, _tool = build_echo_agent(
                name="Strict context agent",
                model=DeterministicToolModel(),
                on_invoke_tool=invoke,
                needs_approval=True,
            )
            return agent

        initial_agent = build_agent()
        paused = Runner.run_streamed(
            initial_agent,
            "Use the approved tool.",
            context=NonMappingContext(project_id="project-strict"),
            run_config=sdk_runtime.build_run_config(),
        )
        await sdk_runtime.drain_stream(paused)
        state_json = json.loads(
            json.dumps(
                paused.to_state().to_json(
                    context_serializer=lambda context: {
                        "project_id": context.project_id
                    },
                    strict_context=True,
                )
            )
        )

        with self.assertRaisesRegex(
            UserError,
            "provide context_deserializer or context_override",
        ):
            await sdk_runtime.restore_run_state(build_agent(), state_json)


if __name__ == "__main__":
    unittest.main()
