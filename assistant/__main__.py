"""Command-line entrypoint for the inherited-stdio sidecar."""

from .protocol_v1_app import run


if __name__ == "__main__":
    run()
