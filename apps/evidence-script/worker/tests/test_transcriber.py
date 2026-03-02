"""Tests for transcriber.py EvidenceTranscriber."""

import os
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from backends.base import TranscriptionResult, TranscriptionSegment


class TestEvidenceTranscriberInit:
    """Tests for EvidenceTranscriber initialization."""

    def test_init_with_backend(self):
        """Test initialization with explicit backend."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()

        transcriber = EvidenceTranscriber(backend=mock_backend)

        assert transcriber.backend is mock_backend

    @patch("transcriber.get_backend")
    def test_init_without_backend_uses_factory(self, mock_get_backend):
        """Test initialization without backend uses get_backend()."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        mock_get_backend.return_value = mock_backend

        transcriber = EvidenceTranscriber()

        mock_get_backend.assert_called_once()
        assert transcriber.backend is mock_backend


class TestEvidenceTranscriberFillerPrompt:
    """Tests for EvidenceTranscriber filler prompt methods."""

    def test_get_filler_prompt_japanese(self):
        """Test Japanese filler prompt."""
        from transcriber import EvidenceTranscriber

        transcriber = EvidenceTranscriber(backend=MagicMock())

        prompt = transcriber._get_filler_prompt("ja")

        assert "えーと" in prompt
        assert "あのー" in prompt
        assert "うーん" in prompt

    def test_get_filler_prompt_english(self):
        """Test English filler prompt."""
        from transcriber import EvidenceTranscriber

        transcriber = EvidenceTranscriber(backend=MagicMock())

        prompt = transcriber._get_filler_prompt("en")

        assert "um" in prompt
        assert "uh" in prompt
        assert "like" in prompt

    def test_get_filler_prompt_unknown_language(self):
        """Test filler prompt for unknown language returns empty."""
        from transcriber import EvidenceTranscriber

        transcriber = EvidenceTranscriber(backend=MagicMock())

        prompt = transcriber._get_filler_prompt("unknown")

        assert prompt == ""


class TestEvidenceTranscriberTranscribe:
    """Tests for EvidenceTranscriber.transcribe method."""

    def test_transcribe_delegates_to_backend(self):
        """Test that transcribe delegates to backend."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        expected_result = TranscriptionResult(
            segments=[TranscriptionSegment("SPEAKER_00", 0.0, 5.0, "Hello")],
            speakers=["SPEAKER_00"],
            duration_seconds=5.0,
            language="en",
        )
        mock_backend.transcribe.return_value = expected_result

        transcriber = EvidenceTranscriber(backend=mock_backend)

        result = transcriber.transcribe(
            audio_path="/test/audio.wav",
            language="en",
            min_speakers=1,
            max_speakers=2,
        )

        assert result is expected_result
        mock_backend.transcribe.assert_called_once()

    def test_transcribe_passes_filler_prompt(self):
        """Test that transcribe passes filler prompt to backend."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        mock_backend.transcribe.return_value = TranscriptionResult(
            segments=[], speakers=[], duration_seconds=0.0, language="ja"
        )

        transcriber = EvidenceTranscriber(backend=mock_backend)

        transcriber.transcribe(audio_path="/test/audio.wav", language="ja")

        call_kwargs = mock_backend.transcribe.call_args[1]
        assert "えーと" in call_kwargs["filler_prompt"]

    def test_transcribe_passes_speaker_params(self):
        """Test that transcribe passes speaker parameters."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        mock_backend.transcribe.return_value = TranscriptionResult(
            segments=[], speakers=[], duration_seconds=0.0, language="en"
        )

        transcriber = EvidenceTranscriber(backend=mock_backend)

        transcriber.transcribe(
            audio_path="/test/audio.wav",
            language="en",
            min_speakers=2,
            max_speakers=5,
        )

        call_kwargs = mock_backend.transcribe.call_args[1]
        assert call_kwargs["min_speakers"] == 2
        assert call_kwargs["max_speakers"] == 5


class TestEvidenceTranscriberExtractAudio:
    """Tests for EvidenceTranscriber.extract_audio method."""

    @patch("transcriber.subprocess.run")
    def test_extract_audio_calls_ffmpeg(self, mock_run):
        """Test that extract_audio calls ffmpeg correctly."""
        from transcriber import EvidenceTranscriber

        mock_run.return_value = MagicMock(returncode=0)

        transcriber = EvidenceTranscriber(backend=MagicMock())

        with tempfile.TemporaryDirectory() as tmpdir:
            output_path = os.path.join(tmpdir, "output.wav")

            result = transcriber.extract_audio("/input/video.mp4", output_path)

            assert result == output_path
            mock_run.assert_called_once()

            # Check ffmpeg command
            call_args = mock_run.call_args[0][0]
            assert call_args[0] == "ffmpeg"
            assert "-i" in call_args
            assert "/input/video.mp4" in call_args
            assert "-vn" in call_args  # No video
            assert "16000" in call_args  # 16kHz sample rate

    @patch("transcriber.subprocess.run")
    def test_extract_audio_creates_parent_dirs(self, mock_run):
        """Test that extract_audio creates parent directories."""
        from transcriber import EvidenceTranscriber

        mock_run.return_value = MagicMock(returncode=0)

        transcriber = EvidenceTranscriber(backend=MagicMock())

        with tempfile.TemporaryDirectory() as tmpdir:
            output_path = os.path.join(tmpdir, "nested", "dir", "output.wav")

            transcriber.extract_audio("/input/video.mp4", output_path)

            assert Path(output_path).parent.exists()

    @patch("transcriber.subprocess.run")
    def test_extract_audio_raises_on_ffmpeg_error(self, mock_run):
        """Test that extract_audio raises on ffmpeg error."""
        from transcriber import EvidenceTranscriber

        mock_run.return_value = MagicMock(
            returncode=1,
            stderr="FFmpeg error message",
        )

        transcriber = EvidenceTranscriber(backend=MagicMock())

        with tempfile.TemporaryDirectory() as tmpdir:
            output_path = os.path.join(tmpdir, "output.wav")

            with pytest.raises(RuntimeError) as exc_info:
                transcriber.extract_audio("/input/video.mp4", output_path)

            assert "FFmpeg error" in str(exc_info.value)


class TestEvidenceTranscriberTranscribeVideo:
    """Tests for EvidenceTranscriber.transcribe_video method."""

    @patch("transcriber.Path.unlink")
    def test_transcribe_video_extracts_and_transcribes(self, mock_unlink):
        """Test that transcribe_video extracts audio and transcribes."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        expected_result = TranscriptionResult(
            segments=[TranscriptionSegment("SPEAKER_00", 0.0, 10.0, "Video content")],
            speakers=["SPEAKER_00"],
            duration_seconds=10.0,
            language="ja",
        )
        mock_backend.transcribe.return_value = expected_result

        transcriber = EvidenceTranscriber(backend=mock_backend)

        with patch.object(transcriber, "extract_audio") as mock_extract:
            mock_extract.return_value = "/data/temp/video.wav"

            result = transcriber.transcribe_video(
                video_path="/input/video.mp4",
                language="ja",
                max_speakers=2,
            )

            assert result is expected_result
            mock_extract.assert_called_once()
            mock_backend.transcribe.assert_called_once()

    @patch("transcriber.Path.unlink")
    def test_transcribe_video_cleans_up_temp_file(self, mock_unlink):
        """Test that transcribe_video cleans up temp audio file."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        mock_backend.transcribe.return_value = TranscriptionResult(
            segments=[], speakers=[], duration_seconds=0.0, language="ja"
        )

        transcriber = EvidenceTranscriber(backend=mock_backend)

        with patch.object(transcriber, "extract_audio") as mock_extract:
            mock_extract.return_value = "/data/temp/video.wav"

            transcriber.transcribe_video("/input/video.mp4")

            # Verify cleanup was attempted
            mock_unlink.assert_called()

    @patch("transcriber.Path.unlink")
    def test_transcribe_video_cleans_up_on_error(self, mock_unlink):
        """Test that transcribe_video cleans up even on error."""
        from transcriber import EvidenceTranscriber

        mock_backend = MagicMock()
        mock_backend.transcribe.side_effect = RuntimeError("Transcription failed")

        transcriber = EvidenceTranscriber(backend=mock_backend)

        with patch.object(transcriber, "extract_audio") as mock_extract:
            mock_extract.return_value = "/data/temp/video.wav"

            with pytest.raises(RuntimeError):
                transcriber.transcribe_video("/input/video.mp4")

            # Verify cleanup was still attempted
            mock_unlink.assert_called()
