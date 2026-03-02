"""
WhisperX Backend for STT

Uses WhisperX for:
1. Transcription with filler preservation (initial_prompt)
2. Word-level alignment
3. Speaker diarization (Pyannote)
"""

import logging
import os

import whisperx

from .base import TranscriptionResult, TranscriptionSegment

logger = logging.getLogger(__name__)


class WhisperXBackend:
    """WhisperX-based STT backend with speaker diarization."""

    def __init__(
        self,
        device: str = "cuda",
        compute_type: str = "float16",
        model_size: str = "large-v3",
    ):
        self.device = device
        self.compute_type = compute_type
        self.model_size = model_size
        self.hf_token = os.environ.get("HF_TOKEN")

        if not self.hf_token:
            logger.warning(
                "HF_TOKEN not set. Speaker diarization will be disabled."
            )

    def transcribe(
        self,
        audio_path: str,
        language: str,
        filler_prompt: str,
        min_speakers: int | None,
        max_speakers: int | None,
    ) -> TranscriptionResult:
        """
        Transcribe audio with filler preservation and speaker diarization.

        Args:
            audio_path: Path to audio file (WAV/MP3/etc)
            language: Language code (ja, en)
            filler_prompt: Prompt to encourage filler word preservation
            min_speakers: Minimum expected speakers (optional)
            max_speakers: Maximum expected speakers (optional)

        Returns:
            TranscriptionResult with speaker-labeled segments
        """
        logger.info(f"[WhisperX] Starting transcription: {audio_path}")

        # Step 1: Load audio
        audio = whisperx.load_audio(audio_path)
        duration = len(audio) / 16000  # 16kHz sample rate

        # Step 2: Transcription with filler preservation
        logger.info("[WhisperX] Loading Whisper model...")
        model = whisperx.load_model(
            self.model_size,
            self.device,
            compute_type=self.compute_type,
        )

        if filler_prompt:
            logger.info(f"[WhisperX] Using filler prompt: {filler_prompt[:50]}...")

        result = model.transcribe(
            audio,
            language=language,
            initial_prompt=filler_prompt,
            word_timestamps=True,
        )

        # Free VRAM
        del model

        # Step 3: Forced alignment for word-level timestamps
        logger.info("[WhisperX] Aligning transcription...")
        align_model, metadata = whisperx.load_align_model(
            language_code=language,
            device=self.device,
        )

        result = whisperx.align(
            result["segments"],
            align_model,
            metadata,
            audio,
            self.device,
            return_char_alignments=False,
        )

        del align_model

        # Step 4: Speaker diarization (if HF token available)
        speakers = []
        if self.hf_token:
            logger.info("[WhisperX] Running speaker diarization...")
            try:
                diarize_model = whisperx.DiarizationPipeline(
                    use_auth_token=self.hf_token,
                    device=self.device,
                )

                diarize_segments = diarize_model(
                    audio_path,
                    min_speakers=min_speakers,
                    max_speakers=max_speakers,
                )

                # Assign speakers to words
                result = whisperx.assign_word_speakers(diarize_segments, result)

                # Extract unique speakers
                for segment in result.get("segments", []):
                    speaker = segment.get("speaker", "UNKNOWN")
                    if speaker not in speakers:
                        speakers.append(speaker)

            except Exception as e:
                logger.error(f"[WhisperX] Diarization failed: {e}")
                # Continue without diarization
        else:
            logger.warning("[WhisperX] Skipping diarization (no HF_TOKEN)")

        # Step 5: Build output
        segments = []
        for seg in result.get("segments", []):
            speaker = seg.get("speaker", "SPEAKER_00")
            segments.append(
                TranscriptionSegment(
                    speaker=speaker,
                    start=round(seg["start"], 2),
                    end=round(seg["end"], 2),
                    text=seg["text"].strip(),
                )
            )

        if not speakers:
            speakers = ["SPEAKER_00"]

        logger.info(
            f"[WhisperX] Transcription complete: {len(segments)} segments, "
            f"{len(speakers)} speakers, {duration:.1f}s"
        )

        return TranscriptionResult(
            segments=segments,
            speakers=speakers,
            duration_seconds=round(duration, 2),
            language=language,
        )
