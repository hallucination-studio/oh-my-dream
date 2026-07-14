from __future__ import annotations

import unittest


class AgentLimitTests(unittest.TestCase):
    def test_production_loop_has_explicit_turn_and_parallel_call_limits(self) -> None:
        from assistant.sdk_runtime import SDK_MAX_TURNS, build_model_settings

        self.assertGreater(SDK_MAX_TURNS, 10)
        self.assertIs(build_model_settings().parallel_tool_calls, False)


if __name__ == "__main__":
    unittest.main()
