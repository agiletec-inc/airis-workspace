"""Tests for backends/__init__.py factory function."""

import os
from unittest.mock import patch

import pytest


class TestGetBackend:
    """Tests for get_backend factory function."""

    def test_get_whisperx_backend_explicit(self):
        """Test getting WhisperX backend by name."""
        from backends import get_backend
        from backends.whisperx import WhisperXBackend

        backend = get_backend("whisperx")

        assert isinstance(backend, WhisperXBackend)

    def test_get_kotoba_backend_explicit(self):
        """Test getting Kotoba backend by name."""
        from backends import get_backend
        from backends.kotoba import KotobaBackend

        backend = get_backend("kotoba")

        assert isinstance(backend, KotobaBackend)

    def test_get_backend_from_env_whisperx(self):
        """Test getting backend from STT_BACKEND env var (whisperx)."""
        from backends.whisperx import WhisperXBackend

        with patch.dict(os.environ, {"STT_BACKEND": "whisperx"}):
            from backends import get_backend

            backend = get_backend()

            assert isinstance(backend, WhisperXBackend)

    def test_get_backend_from_env_kotoba(self):
        """Test getting backend from STT_BACKEND env var (kotoba)."""
        from backends.kotoba import KotobaBackend

        with patch.dict(os.environ, {"STT_BACKEND": "kotoba"}):
            from backends import get_backend

            backend = get_backend()

            assert isinstance(backend, KotobaBackend)

    def test_get_backend_default_is_whisperx(self):
        """Test that default backend is whisperx."""
        from backends.whisperx import WhisperXBackend

        with patch.dict(os.environ, {}, clear=True):
            # Remove STT_BACKEND if set
            os.environ.pop("STT_BACKEND", None)
            from backends import get_backend

            backend = get_backend()

            assert isinstance(backend, WhisperXBackend)

    def test_get_backend_unknown_raises_error(self):
        """Test that unknown backend name raises ValueError."""
        from backends import get_backend

        with pytest.raises(ValueError) as exc_info:
            get_backend("unknown_backend")

        assert "Unknown backend" in str(exc_info.value)
        assert "unknown_backend" in str(exc_info.value)

    def test_get_backend_respects_device_env(self):
        """Test that backend respects DEVICE env var."""
        with patch.dict(os.environ, {"DEVICE": "cpu"}):
            from backends import get_backend

            backend = get_backend("whisperx")

            assert backend.device == "cpu"

    def test_get_backend_respects_compute_type_env(self):
        """Test that backend respects COMPUTE_TYPE env var."""
        with patch.dict(os.environ, {"COMPUTE_TYPE": "int8"}):
            from backends import get_backend

            backend = get_backend("whisperx")

            assert backend.compute_type == "int8"

    def test_get_backend_respects_whisper_model_env(self):
        """Test that WhisperX backend respects WHISPER_MODEL env var."""
        with patch.dict(os.environ, {"WHISPER_MODEL": "medium"}):
            from backends import get_backend

            backend = get_backend("whisperx")

            assert backend.model_size == "medium"
