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


if __name__ == "__main__":
    unittest.main()
