"""
Evidence Transcriber - Court-Ready Transcription with Filler Preservation

Uses pluggable STT backends (WhisperX, Kotoba) for:
1. Transcription with filler preservation
2. Word-level alignment
3. Speaker diarization

Output: JSON with speaker-labeled segments including fillers
"""

import logging
import subprocess
from pathlib import Path

from backends import STTBackend, TranscriptionResult, get_backend

logger = logging.getLogger(__name__)


# Re-export for backward compatibility
TranscriptSegment = __import__(
    "backends.base", fromlist=["TranscriptionSegment"]
).TranscriptionSegment


class EvidenceTranscriber:
    """Transcriber optimized for court evidence with filler preservation."""

    # Japanese filler prompt to encourage filler retention
    FILLER_PROMPT_JA = (
        "えーと、あのー、うーん、えー、その、まあ、なんか、ちょっと、"
        "はい、いや、そうですね、あ、ああ、うん、ええ、おー、"
        "ですから、なので、つまり、要するに、"
        "えっと、あのですね、ちょっとですね"
    )

    # English filler prompt
    FILLER_PROMPT_EN = (
        "um, uh, er, ah, like, you know, I mean, so, well, "
        "basically, actually, honestly, literally, right, okay"
    )

    def __init__(self, backend: STTBackend | None = None):
        """
        Initialize the transcriber with a specific backend.

        Args:
            backend: STTBackend instance. If None, uses get_backend()
                    which respects STT_BACKEND environment variable.
        """
        self.backend = backend or get_backend()

    def _get_filler_prompt(self, language: str) -> str:
        """Get language-appropriate filler prompt."""
        prompts = {
            "ja": self.FILLER_PROMPT_JA,
            "en": self.FILLER_PROMPT_EN,
        }
        return prompts.get(language, "")

    def extract_audio(self, input_path: str, output_path: str) -> str:
        """Extract audio from video file using FFmpeg."""
        output_path_obj = Path(output_path)
        output_path_obj.parent.mkdir(parents=True, exist_ok=True)

        cmd = [
            "ffmpeg",
            "-y",
            "-i", input_path,
            "-vn",                    # No video
            "-acodec", "pcm_s16le",   # 16-bit PCM
            "-ar", "16000",           # 16kHz sample rate (optimal for Whisper)
            "-ac", "1",               # Mono
            str(output_path_obj),
        ]

        logger.info(f"Extracting audio: {input_path} -> {output_path}")

        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(f"FFmpeg error: {result.stderr}")

        return str(output_path_obj)

    def transcribe(
        self,
        audio_path: str,
        language: str = "ja",
        min_speakers: int | None = None,
        max_speakers: int | None = None,
    ) -> TranscriptionResult:
        """
        Transcribe audio with filler preservation and speaker diarization.

        Args:
            audio_path: Path to audio file (WAV/MP3/etc)
            language: Language code (ja, en)
            min_speakers: Minimum expected speakers (optional)
            max_speakers: Maximum expected speakers (optional)

        Returns:
            TranscriptionResult with speaker-labeled segments
        """
        logger.info(f"Starting transcription: {audio_path}")

        filler_prompt = self._get_filler_prompt(language)

        return self.backend.transcribe(
            audio_path=audio_path,
            language=language,
            filler_prompt=filler_prompt,
            min_speakers=min_speakers,
            max_speakers=max_speakers,
        )

    def transcribe_video(
        self,
        video_path: str,
        language: str = "ja",
        min_speakers: int | None = None,
        max_speakers: int | None = None,
    ) -> TranscriptionResult:
        """
        Transcribe video file (extracts audio first).

        Args:
            video_path: Path to video file (MP4/MOV/etc)
            language: Language code
            min_speakers: Minimum expected speakers
            max_speakers: Maximum expected speakers

        Returns:
            TranscriptionResult
        """
        # Extract audio to temp file
        video_path_obj = Path(video_path)
        audio_path = f"/data/temp/{video_path_obj.stem}.wav"

        self.extract_audio(str(video_path_obj), audio_path)

        try:
            return self.transcribe(
                audio_path,
                language=language,
                min_speakers=min_speakers,
                max_speakers=max_speakers,
            )
        finally:
            # Cleanup temp audio
            try:
                Path(audio_path).unlink()
            except OSError:
                pass
