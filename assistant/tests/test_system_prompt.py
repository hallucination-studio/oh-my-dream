from __future__ import annotations

import unittest

from assistant.system_prompt import build_system_prompt


class SystemPromptTests(unittest.TestCase):
    def test_prompt_allows_sdk_owned_iteration_and_correction(self) -> None:
        prompt = build_system_prompt()

        self.assertNotIn("Use workflow_apply_patch once", prompt)
        self.assertNotIn("do not retry", prompt.lower())
        self.assertIn("Keep using tools", prompt)
        self.assertIn("choose the next creative step", prompt)
        self.assertIn("structured tool result", prompt)
        self.assertIn("workflow_evaluate_patch", prompt)

    def test_prompt_treats_production_plan_as_agent_memory_not_a_queue(self) -> None:
        prompt = build_system_prompt()

        self.assertIn("production_plan_get", prompt)
        self.assertIn("Agent-owned memory", prompt)
        self.assertNotIn("claim_next", prompt)
        self.assertNotIn("Product chooses the next", prompt)


if __name__ == "__main__":
    unittest.main()
