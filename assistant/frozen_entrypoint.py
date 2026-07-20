"""Frozen production entrypoint for invocation and provider control."""

import os

from assistant.protocol_v1_app import run as run_protocol
from assistant.provider_control import run as run_provider_control


if __name__ == "__main__":
    if os.environ.get("OH_MY_DREAM_ASSISTANT_MODE") == "provider_control":
        run_provider_control()
    else:
        run_protocol()
