"""Tests for backends/base.py data structures."""

import pytest

from backends.base import TranscriptionSegment, TranscriptionResult


class TestTranscriptionSegment:
    """Tests for TranscriptionSegment dataclass."""

    def test_create_segment(self):
        """Test creating a transcription segment."""
        segment = TranscriptionSegment(
            speaker="SPEAKER_00",
            start=0.0,
            end=5.5,
            text="こんにちは",
        )

        assert segment.speaker == "SPEAKER_00"
        assert segment.start == 0.0
        assert segment.end == 5.5
        assert segment.text == "こんにちは"

    def test_segment_with_filler(self):
        """Test segment containing filler words."""
        segment = TranscriptionSegment(
            speaker="SPEAKER_01",
            start=10.2,
            end=15.8,
            text="えーと、あのー、そうですね",
        )

        assert "えーと" in segment.text
        assert "あのー" in segment.text

    def test_segment_equality(self):
        """Test segment equality comparison."""
        seg1 = TranscriptionSegment("A", 0.0, 1.0, "test")
        seg2 = TranscriptionSegment("A", 0.0, 1.0, "test")
        seg3 = TranscriptionSegment("B", 0.0, 1.0, "test")

        assert seg1 == seg2
        assert seg1 != seg3


class TestTranscriptionResult:
    """Tests for TranscriptionResult dataclass."""

    def test_create_result(self):
        """Test creating a transcription result."""
        segments = [
            TranscriptionSegment("SPEAKER_00", 0.0, 5.0, "Hello"),
            TranscriptionSegment("SPEAKER_01", 5.0, 10.0, "Hi there"),
        ]

        result = TranscriptionResult(
            segments=segments,
            speakers=["SPEAKER_00", "SPEAKER_01"],
            duration_seconds=10.0,
            language="en",
        )

        assert len(result.segments) == 2
        assert len(result.speakers) == 2
        assert result.duration_seconds == 10.0
        assert result.language == "en"

    def test_empty_result(self):
        """Test creating an empty transcription result."""
        result = TranscriptionResult(
            segments=[],
            speakers=["SPEAKER_00"],
            duration_seconds=0.0,
            language="ja",
        )

        assert len(result.segments) == 0
        assert result.duration_seconds == 0.0

    def test_result_with_multiple_speakers(self):
        """Test result with multiple speakers."""
        segments = [
            TranscriptionSegment("SPEAKER_00", 0.0, 3.0, "First speaker"),
            TranscriptionSegment("SPEAKER_01", 3.0, 6.0, "Second speaker"),
            TranscriptionSegment("SPEAKER_02", 6.0, 9.0, "Third speaker"),
        ]

        result = TranscriptionResult(
            segments=segments,
            speakers=["SPEAKER_00", "SPEAKER_01", "SPEAKER_02"],
            duration_seconds=9.0,
            language="en",
        )

        assert len(result.speakers) == 3
        assert "SPEAKER_02" in result.speakers
