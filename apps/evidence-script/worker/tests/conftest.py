"""Pytest configuration and shared fixtures."""

import os
import sys
from pathlib import Path

import pytest

# Add worker directory to Python path
worker_dir = Path(__file__).parent.parent
sys.path.insert(0, str(worker_dir))


@pytest.fixture(autouse=True)
def reset_env():
    """Reset environment variables between tests."""
    original_env = os.environ.copy()
    yield
    os.environ.clear()
    os.environ.update(original_env)


@pytest.fixture
def sample_transcription_result():
    """Create a sample TranscriptionResult for testing."""
    from backends.base import TranscriptionResult, TranscriptionSegment

    return TranscriptionResult(
        segments=[
            TranscriptionSegment("SPEAKER_00", 0.0, 5.0, "えーと、こんにちは"),
            TranscriptionSegment("SPEAKER_01", 5.0, 10.0, "あのー、はい、こんにちは"),
        ],
        speakers=["SPEAKER_00", "SPEAKER_01"],
        duration_seconds=10.0,
        language="ja",
    )


@pytest.fixture
def mock_backend():
    """Create a mock STT backend."""
    from unittest.mock import MagicMock

    from backends.base import TranscriptionResult, TranscriptionSegment

    backend = MagicMock()
    backend.transcribe.return_value = TranscriptionResult(
        segments=[TranscriptionSegment("SPEAKER_00", 0.0, 1.0, "Test")],
        speakers=["SPEAKER_00"],
        duration_seconds=1.0,
        language="ja",
    )
    return backend
