from __future__ import annotations

from assistant.stdio_protocol import JsonValue


class RecordingWriter:
    def __init__(
        self,
        *,
        short_write_bytes: int | None = None,
        fail_flush: bool = False,
    ) -> None:
        self.chunks: list[bytes] = []
        self.flush_count = 0
        self.write_count = 0
        self._short_write_bytes = short_write_bytes
        self._fail_flush = fail_flush

    def write(self, data: bytes) -> int:
        self.write_count += 1
        accepted = data
        if self._short_write_bytes is not None:
            accepted = data[: self._short_write_bytes]
        self.chunks.append(accepted)
        return len(accepted)

    def flush(self) -> None:
        self.flush_count += 1
        if self._fail_flush:
            raise OSError("injected flush failure")

    @property
    def bytes(self) -> bytes:
        return b"".join(self.chunks)


def frame_bytes(sequence: int, kind: str = "invoke", payload: str = "{}") -> bytes:
    return (
        f'{{"protocol_version":1,"sequence":{sequence},"kind":"{kind}",'
        f'"payload":{payload}}}\n'
    ).encode()


def nested_payload_at_depth(deepest_depth: int) -> dict[str, JsonValue]:
    if deepest_depth < 1:
        raise ValueError("payload object starts at frame depth 1")
    payload: dict[str, JsonValue] = {"scalar_at_next_depth": "accepted"}
    for _ in range(deepest_depth - 1):
        payload = {"nested": payload}
    return payload
