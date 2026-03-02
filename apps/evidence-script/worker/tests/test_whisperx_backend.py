"""Tests for backends/whisperx.py WhisperXBackend."""

import os
from unittest.mock import MagicMock, patch

import pytest

from backends.base import TranscriptionResult, TranscriptionSegment


class TestWhisperXBackendInit:
    """Tests for WhisperXBackend initialization."""

    def test_init_default_values(self):
        """Test default initialization values."""
        from backends.whisperx import WhisperXBackend

        backend = WhisperXBackend()

        assert backend.device == "cuda"
        assert backend.compute_type == "float16"
        assert backend.model_size == "large-v3"

    def test_init_custom_values(self):
        """Test custom initialization values."""
        from backends.whisperx import WhisperXBackend

        backend = WhisperXBackend(
            device="cpu",
            compute_type="int8",
            model_size="medium",
        )

        assert backend.device == "cpu"
        assert backend.compute_type == "int8"
        assert backend.model_size == "medium"

    def test_init_reads_hf_token(self):
        """Test that HF_TOKEN is read from environment."""
        from backends.whisperx import WhisperXBackend

        with patch.dict(os.environ, {"HF_TOKEN": "test_token"}):
            backend = WhisperXBackend()

            assert backend.hf_token == "test_token"

    def test_init_warns_without_hf_token(self, caplog):
        """Test warning when HF_TOKEN is not set."""
        with patch.dict(os.environ, {}, clear=True):
            os.environ.pop("HF_TOKEN", None)

            from backends.whisperx import WhisperXBackend

            backend = WhisperXBackend()

            assert backend.hf_token is None


class TestWhisperXBackendTranscribe:
    """Tests for WhisperXBackend.transcribe method."""

    @patch("backends.whisperx.whisperx")
    def test_transcribe_returns_result(self, mock_whisperx):
        """Test that transcribe returns TranscriptionResult."""
        from backends.whisperx import WhisperXBackend

        # Mock whisperx functions
        mock_audio = MagicMock()
        mock_audio.__len__ = MagicMock(return_value=160000)  # 10 seconds at 16kHz
        mock_whisperx.load_audio.return_value = mock_audio

        mock_model = MagicMock()
        mock_model.transcribe.return_value = {
            "segments": [
                {"start": 0.0, "end": 5.0, "text": "Hello world"},
            ]
        }
        mock_whisperx.load_model.return_value = mock_model

        mock_align_model = MagicMock()
        mock_whisperx.load_align_model.return_value = (mock_align_model, {})
        mock_whisperx.align.return_value = {
            "segments": [
                {"start": 0.0, "end": 5.0, "text": "Hello world", "speaker": "SPEAKER_00"},
            ]
        }

        backend = WhisperXBackend(device="cpu")
        backend.hf_token = None  # Disable diarization for test

        result = backend.transcribe(
            audio_path="/test/audio.wav",
            language="en",
            filler_prompt="um, uh",
            min_speakers=None,
            max_speakers=None,
        )

        assert isinstance(result, TranscriptionResult)
        assert len(result.segments) == 1
        assert result.segments[0].text == "Hello world"
        assert result.language == "en"

    @patch("backends.whisperx.whisperx")
    def test_transcribe_with_filler_prompt(self, mock_whisperx):
        """Test that filler prompt is passed to transcription."""
        from backends.whisperx import WhisperXBackend

        mock_audio = MagicMock()
        mock_audio.__len__ = MagicMock(return_value=160000)
        mock_whisperx.load_audio.return_value = mock_audio

        mock_model = MagicMock()
        mock_model.transcribe.return_value = {"segments": []}
        mock_whisperx.load_model.return_value = mock_model

        mock_whisperx.load_align_model.return_value = (MagicMock(), {})
        mock_whisperx.align.return_value = {"segments": []}

        backend = WhisperXBackend(device="cpu")
        backend.hf_token = None

        filler_prompt = "えーと、あのー"

        backend.transcribe(
            audio_path="/test/audio.wav",
            language="ja",
            filler_prompt=filler_prompt,
            min_speakers=None,
            max_speakers=None,
        )

        # Verify filler prompt was passed
        mock_model.transcribe.assert_called_once()
        call_kwargs = mock_model.transcribe.call_args[1]
        assert call_kwargs["initial_prompt"] == filler_prompt

    @patch("backends.whisperx.whisperx")
    def test_transcribe_with_diarization(self, mock_whisperx):
        """Test transcription with speaker diarization."""
        from backends.whisperx import WhisperXBackend

        mock_audio = MagicMock()
        mock_audio.__len__ = MagicMock(return_value=320000)  # 20 seconds
        mock_whisperx.load_audio.return_value = mock_audio

        mock_model = MagicMock()
        mock_model.transcribe.return_value = {"segments": []}
        mock_whisperx.load_model.return_value = mock_model

        mock_whisperx.load_align_model.return_value = (MagicMock(), {})

        # Mock aligned result with speakers
        mock_whisperx.align.return_value = {
            "segments": [
                {"start": 0.0, "end": 5.0, "text": "Speaker one", "speaker": "SPEAKER_00"},
                {"start": 5.0, "end": 10.0, "text": "Speaker two", "speaker": "SPEAKER_01"},
            ]
        }

        # Mock diarization
        mock_diarize = MagicMock()
        mock_diarize.return_value = MagicMock()
        mock_whisperx.DiarizationPipeline.return_value = mock_diarize

        mock_whisperx.assign_word_speakers.return_value = {
            "segments": [
                {"start": 0.0, "end": 5.0, "text": "Speaker one", "speaker": "SPEAKER_00"},
                {"start": 5.0, "end": 10.0, "text": "Speaker two", "speaker": "SPEAKER_01"},
            ]
        }

        with patch.dict(os.environ, {"HF_TOKEN": "test_token"}):
            backend = WhisperXBackend(device="cpu")

            result = backend.transcribe(
                audio_path="/test/audio.wav",
                language="en",
                filler_prompt="",
                min_speakers=2,
                max_speakers=2,
            )

        assert len(result.speakers) == 2
        assert "SPEAKER_00" in result.speakers
        assert "SPEAKER_01" in result.speakers

    @patch("backends.whisperx.whisperx")
    def test_transcribe_calculates_duration(self, mock_whisperx):
        """Test that duration is calculated correctly."""
        from backends.whisperx import WhisperXBackend

        # 5 seconds of audio at 16kHz
        mock_audio = MagicMock()
        mock_audio.__len__ = MagicMock(return_value=80000)
        mock_whisperx.load_audio.return_value = mock_audio

        mock_model = MagicMock()
        mock_model.transcribe.return_value = {"segments": []}
        mock_whisperx.load_model.return_value = mock_model

        mock_whisperx.load_align_model.return_value = (MagicMock(), {})
        mock_whisperx.align.return_value = {"segments": []}

        backend = WhisperXBackend(device="cpu")
        backend.hf_token = None

        result = backend.transcribe(
            audio_path="/test/audio.wav",
            language="en",
            filler_prompt="",
            min_speakers=None,
            max_speakers=None,
        )

        assert result.duration_seconds == 5.0
