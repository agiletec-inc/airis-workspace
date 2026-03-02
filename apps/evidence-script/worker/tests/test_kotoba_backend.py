"""Tests for backends/kotoba.py KotobaBackend."""

import os
from unittest.mock import MagicMock, patch

import pytest
import torch

from backends.base import TranscriptionResult, TranscriptionSegment


class TestKotobaBackendInit:
    """Tests for KotobaBackend initialization."""

    def test_init_default_values(self):
        """Test default initialization values."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend()

        assert backend.device == "cuda"
        assert backend.torch_dtype == torch.float16

    def test_init_float16_compute_type(self):
        """Test float16 compute type."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(compute_type="float16")

        assert backend.torch_dtype == torch.float16
        assert backend.load_in_8bit is False

    def test_init_bfloat16_compute_type(self):
        """Test bfloat16 compute type."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(compute_type="bfloat16")

        assert backend.torch_dtype == torch.bfloat16

    def test_init_int8_compute_type(self):
        """Test int8 compute type."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(compute_type="int8")

        assert backend.load_in_8bit is True

    def test_init_float32_compute_type(self):
        """Test float32 compute type."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(compute_type="float32")

        assert backend.torch_dtype == torch.float32
        assert backend.load_in_8bit is False

    def test_init_reads_hf_token(self):
        """Test that HF_TOKEN is read from environment."""
        from backends.kotoba import KotobaBackend

        with patch.dict(os.environ, {"HF_TOKEN": "test_token"}):
            backend = KotobaBackend()

            assert backend.hf_token == "test_token"

    def test_model_id_is_correct(self):
        """Test that model ID is kotoba-whisper-v2.2."""
        from backends.kotoba import KotobaBackend

        assert KotobaBackend.MODEL_ID == "kotoba-tech/kotoba-whisper-v2.2"


class TestKotobaBackendAssignSpeakers:
    """Tests for KotobaBackend._assign_speakers method."""

    def test_assign_speakers_without_diarization(self):
        """Test speaker assignment without diarization data."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(device="cpu")

        chunks = [
            {"text": "Hello", "timestamp": (0.0, 2.0)},
            {"text": "World", "timestamp": (2.0, 4.0)},
        ]

        segments, speakers = backend._assign_speakers(chunks, None)

        assert len(segments) == 2
        assert all(seg.speaker == "SPEAKER_00" for seg in segments)
        assert speakers == ["SPEAKER_00"]

    def test_assign_speakers_with_diarization(self):
        """Test speaker assignment with diarization data."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(device="cpu")

        chunks = [
            {"text": "Hello", "timestamp": (0.0, 2.0)},
            {"text": "Hi there", "timestamp": (2.0, 4.0)},
        ]

        diarization = [
            {"start": 0.0, "end": 2.0, "speaker": "SPEAKER_00"},
            {"start": 2.0, "end": 4.0, "speaker": "SPEAKER_01"},
        ]

        segments, speakers = backend._assign_speakers(chunks, diarization)

        assert len(segments) == 2
        assert segments[0].speaker == "SPEAKER_00"
        assert segments[1].speaker == "SPEAKER_01"
        assert set(speakers) == {"SPEAKER_00", "SPEAKER_01"}

    def test_assign_speakers_empty_text_filtered(self):
        """Test that empty text chunks are filtered out."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(device="cpu")

        chunks = [
            {"text": "Hello", "timestamp": (0.0, 2.0)},
            {"text": "", "timestamp": (2.0, 3.0)},
            {"text": "   ", "timestamp": (3.0, 4.0)},
            {"text": "World", "timestamp": (4.0, 6.0)},
        ]

        segments, speakers = backend._assign_speakers(chunks, None)

        assert len(segments) == 2
        assert segments[0].text == "Hello"
        assert segments[1].text == "World"

    def test_assign_speakers_overlap_detection(self):
        """Test speaker assignment with overlapping segments."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(device="cpu")

        chunks = [
            {"text": "Long segment", "timestamp": (0.0, 5.0)},
        ]

        # Diarization with partial overlap
        diarization = [
            {"start": 0.0, "end": 2.0, "speaker": "SPEAKER_00"},
            {"start": 2.0, "end": 5.0, "speaker": "SPEAKER_01"},  # More overlap
        ]

        segments, speakers = backend._assign_speakers(chunks, diarization)

        # Should pick SPEAKER_01 due to more overlap
        assert segments[0].speaker == "SPEAKER_01"


class TestKotobaBackendTranscribe:
    """Tests for KotobaBackend.transcribe method."""

    @patch("backends.kotoba.torchaudio")
    @patch("backends.kotoba.pipeline")
    @patch("backends.kotoba.AutoProcessor")
    @patch("backends.kotoba.AutoModelForSpeechSeq2Seq")
    def test_transcribe_returns_result(
        self, mock_model_cls, mock_processor_cls, mock_pipeline, mock_torchaudio
    ):
        """Test that transcribe returns TranscriptionResult."""
        from backends.kotoba import KotobaBackend

        # Mock model and processor
        mock_model = MagicMock()
        mock_model_cls.from_pretrained.return_value = mock_model

        mock_processor = MagicMock()
        mock_processor_cls.from_pretrained.return_value = mock_processor

        # Mock pipeline
        mock_pipe = MagicMock()
        mock_pipe.return_value = {
            "text": "Hello world",
            "chunks": [
                {"text": "Hello", "timestamp": (0.0, 1.0)},
                {"text": "world", "timestamp": (1.0, 2.0)},
            ],
        }
        mock_pipeline.return_value = mock_pipe

        # Mock torchaudio for duration
        mock_waveform = torch.zeros(1, 32000)  # 2 seconds at 16kHz
        mock_torchaudio.load.return_value = (mock_waveform, 16000)

        backend = KotobaBackend(device="cpu")
        backend.hf_token = None  # Disable diarization

        result = backend.transcribe(
            audio_path="/test/audio.wav",
            language="ja",
            filler_prompt="えーと",
            min_speakers=None,
            max_speakers=None,
        )

        assert isinstance(result, TranscriptionResult)
        assert len(result.segments) == 2
        assert result.language == "ja"

    @patch("backends.kotoba.torchaudio")
    @patch("backends.kotoba.pipeline")
    @patch("backends.kotoba.AutoProcessor")
    @patch("backends.kotoba.AutoModelForSpeechSeq2Seq")
    def test_transcribe_calculates_duration(
        self, mock_model_cls, mock_processor_cls, mock_pipeline, mock_torchaudio
    ):
        """Test that duration is calculated from audio."""
        from backends.kotoba import KotobaBackend

        mock_model_cls.from_pretrained.return_value = MagicMock()
        mock_processor_cls.from_pretrained.return_value = MagicMock()

        mock_pipe = MagicMock()
        mock_pipe.return_value = {"text": "Test", "chunks": []}
        mock_pipeline.return_value = mock_pipe

        # 10 seconds of audio at 16kHz
        mock_waveform = torch.zeros(1, 160000)
        mock_torchaudio.load.return_value = (mock_waveform, 16000)

        backend = KotobaBackend(device="cpu")
        backend.hf_token = None

        result = backend.transcribe(
            audio_path="/test/audio.wav",
            language="ja",
            filler_prompt="",
            min_speakers=None,
            max_speakers=None,
        )

        assert result.duration_seconds == 10.0

    @patch("backends.kotoba.torchaudio")
    @patch("backends.kotoba.pipeline")
    @patch("backends.kotoba.AutoProcessor")
    @patch("backends.kotoba.AutoModelForSpeechSeq2Seq")
    def test_transcribe_fallback_single_segment(
        self, mock_model_cls, mock_processor_cls, mock_pipeline, mock_torchaudio
    ):
        """Test fallback to single segment when no chunks."""
        from backends.kotoba import KotobaBackend

        mock_model_cls.from_pretrained.return_value = MagicMock()
        mock_processor_cls.from_pretrained.return_value = MagicMock()

        # No chunks, only text
        mock_pipe = MagicMock()
        mock_pipe.return_value = {"text": "Full transcription text"}
        mock_pipeline.return_value = mock_pipe

        mock_waveform = torch.zeros(1, 80000)  # 5 seconds
        mock_torchaudio.load.return_value = (mock_waveform, 16000)

        backend = KotobaBackend(device="cpu")
        backend.hf_token = None

        result = backend.transcribe(
            audio_path="/test/audio.wav",
            language="ja",
            filler_prompt="",
            min_speakers=None,
            max_speakers=None,
        )

        assert len(result.segments) == 1
        assert result.segments[0].text == "Full transcription text"
        assert result.segments[0].start == 0.0
        assert result.segments[0].end == 5.0


class TestKotobaBackendDiarization:
    """Tests for KotobaBackend._run_diarization method."""

    def test_diarization_disabled_without_token(self):
        """Test that diarization returns None without HF token."""
        from backends.kotoba import KotobaBackend

        backend = KotobaBackend(device="cpu")
        backend.hf_token = None

        result = backend._run_diarization("/test/audio.wav", None, None)

        assert result is None

    @patch("backends.kotoba.Pipeline")
    def test_diarization_with_token(self, mock_pipeline_cls):
        """Test diarization with HF token."""
        from backends.kotoba import KotobaBackend

        # Mock pyannote pipeline
        mock_pipeline = MagicMock()
        mock_diarization = MagicMock()

        # Mock itertracks to return speaker segments
        mock_turn1 = MagicMock()
        mock_turn1.start = 0.0
        mock_turn1.end = 5.0

        mock_turn2 = MagicMock()
        mock_turn2.start = 5.0
        mock_turn2.end = 10.0

        mock_diarization.itertracks.return_value = [
            (mock_turn1, None, "SPEAKER_00"),
            (mock_turn2, None, "SPEAKER_01"),
        ]

        mock_pipeline.return_value = mock_diarization
        mock_pipeline_cls.from_pretrained.return_value = mock_pipeline

        with patch.dict(os.environ, {"HF_TOKEN": "test_token"}):
            backend = KotobaBackend(device="cpu")

            result = backend._run_diarization("/test/audio.wav", 2, 2)

        assert result is not None
        assert len(result) == 2
        assert result[0]["speaker"] == "SPEAKER_00"
        assert result[1]["speaker"] == "SPEAKER_01"
