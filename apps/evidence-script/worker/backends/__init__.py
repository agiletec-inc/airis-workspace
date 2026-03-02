"""
STT Backends Package

Provides pluggable STT backends via Strategy pattern.
Use get_backend() to get the configured backend.
"""

import os

from .base import STTBackend, TranscriptionResult, TranscriptionSegment

__all__ = [
    "STTBackend",
    "TranscriptionResult",
    "TranscriptionSegment",
    "get_backend",
]


def get_backend(backend_name: str | None = None) -> STTBackend:
    """
    Get STT backend by name or environment variable.

    Args:
        backend_name: Backend name ("whisperx" or "kotoba").
                     If None, uses STT_BACKEND env var (default: whisperx).

    Returns:
        Configured STTBackend instance.

    Raises:
        ValueError: If backend name is unknown.
    """
    name = backend_name or os.environ.get("STT_BACKEND", "whisperx")

    # Get device and compute_type from environment
    device = os.environ.get("DEVICE", "cuda")
    compute_type = os.environ.get("COMPUTE_TYPE", "float16")

    if name == "whisperx":
        from .whisperx import WhisperXBackend

        model_size = os.environ.get("WHISPER_MODEL", "large-v3")
        return WhisperXBackend(
            device=device,
            compute_type=compute_type,
            model_size=model_size,
        )

    elif name == "kotoba":
        from .kotoba import KotobaBackend

        return KotobaBackend(
            device=device,
            compute_type=compute_type,
        )

    else:
        available = ["whisperx", "kotoba"]
        raise ValueError(
            f"Unknown backend: {name}. Available backends: {available}"
        )
