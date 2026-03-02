"""
STT Backend Protocol Definition

This module defines the protocol and data structures for STT backends,
enabling strategy pattern for swappable transcription engines.
"""

from dataclasses import dataclass
from typing import Protocol


@dataclass
class TranscriptionSegment:
    """A single segment of transcribed speech."""

    speaker: str
    start: float
    end: float
    text: str


@dataclass
class TranscriptionResult:
    """Complete transcription result with metadata."""

    segments: list[TranscriptionSegment]
    speakers: list[str]
    duration_seconds: float
    language: str


class STTBackend(Protocol):
    """
    Protocol for Speech-to-Text backends.

    Implementations must provide a transcribe method that takes audio
    and returns a TranscriptionResult with speaker-labeled segments.
    """

    def transcribe(
        self,
        audio_path: str,
        language: str,
        filler_prompt: str,
        min_speakers: int | None,
        max_speakers: int | None,
    ) -> TranscriptionResult:
        """
        Transcribe audio file with speaker diarization.

        Args:
            audio_path: Path to audio file (WAV/MP3/etc)
            language: Language code (ja, en)
            filler_prompt: Prompt to encourage filler word preservation
            min_speakers: Minimum expected speakers (optional)
            max_speakers: Maximum expected speakers (optional)

        Returns:
            TranscriptionResult with speaker-labeled segments
        """
        ...
