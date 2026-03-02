"""
Kotoba-Whisper Backend for STT

Uses kotoba-tech/kotoba-whisper-v2.2 for Japanese transcription
with integrated speaker diarization via pyannote.
"""

import logging
import os

import torch
from transformers import AutoModelForSpeechSeq2Seq, AutoProcessor, pipeline

from .base import TranscriptionResult, TranscriptionSegment

logger = logging.getLogger(__name__)


class KotobaBackend:
    """
    kotoba-tech/kotoba-whisper-v2.2 backend.

    Optimized for Japanese speech recognition with improved
    filler word detection and punctuation restoration.
    """

    MODEL_ID = "kotoba-tech/kotoba-whisper-v2.2"

    def __init__(
        self,
        device: str = "cuda",
        compute_type: str = "float16",
    ):
        self.device = device
        self.hf_token = os.environ.get("HF_TOKEN")

        # Determine torch dtype based on compute_type
        if compute_type in ("float16", "fp16"):
            self.torch_dtype = torch.float16
        elif compute_type in ("bfloat16", "bf16"):
            self.torch_dtype = torch.bfloat16
        elif compute_type in ("int8", "8bit"):
            self.torch_dtype = torch.float16
            self.load_in_8bit = True
        else:
            self.torch_dtype = torch.float32
            self.load_in_8bit = False

        self.load_in_8bit = compute_type in ("int8", "8bit")

        if not self.hf_token:
            logger.warning(
                "HF_TOKEN not set. Speaker diarization will be disabled."
            )

    def _load_pipeline(self):
        """Load the kotoba-whisper pipeline."""
        logger.info(f"[Kotoba] Loading model: {self.MODEL_ID}")

        model_kwargs = {
            "torch_dtype": self.torch_dtype,
            "attn_implementation": "sdpa",
        }

        if self.load_in_8bit:
            model_kwargs["load_in_8bit"] = True

        model = AutoModelForSpeechSeq2Seq.from_pretrained(
            self.MODEL_ID,
            **model_kwargs,
        )

        if not self.load_in_8bit:
            model = model.to(self.device)

        processor = AutoProcessor.from_pretrained(self.MODEL_ID)

        return pipeline(
            "automatic-speech-recognition",
            model=model,
            tokenizer=processor.tokenizer,
            feature_extractor=processor.feature_extractor,
            torch_dtype=self.torch_dtype,
            device=self.device if not self.load_in_8bit else None,
        )

    def _run_diarization(
        self,
        audio_path: str,
        min_speakers: int | None,
        max_speakers: int | None,
    ) -> dict | None:
        """Run speaker diarization using pyannote."""
        if not self.hf_token:
            return None

        try:
            from pyannote.audio import Pipeline

            logger.info("[Kotoba] Running speaker diarization...")

            diarization_pipeline = Pipeline.from_pretrained(
                "pyannote/speaker-diarization-3.1",
                use_auth_token=self.hf_token,
            )
            diarization_pipeline = diarization_pipeline.to(
                torch.device(self.device)
            )

            diarization_kwargs = {}
            if min_speakers is not None:
                diarization_kwargs["min_speakers"] = min_speakers
            if max_speakers is not None:
                diarization_kwargs["max_speakers"] = max_speakers

            diarization = diarization_pipeline(audio_path, **diarization_kwargs)

            # Convert to list of (start, end, speaker) tuples
            segments = []
            for turn, _, speaker in diarization.itertracks(yield_label=True):
                segments.append({
                    "start": turn.start,
                    "end": turn.end,
                    "speaker": speaker,
                })

            return segments

        except Exception as e:
            logger.error(f"[Kotoba] Diarization failed: {e}")
            return None

    def _assign_speakers(
        self,
        transcription_chunks: list[dict],
        diarization_segments: list[dict] | None,
    ) -> tuple[list[TranscriptionSegment], list[str]]:
        """Assign speakers to transcription chunks based on diarization."""
        segments = []
        speakers_set = set()

        for chunk in transcription_chunks:
            text = chunk.get("text", "").strip()
            if not text:
                continue

            start = chunk.get("timestamp", (0, 0))[0] or 0
            end = chunk.get("timestamp", (0, 0))[1] or start

            # Find matching speaker from diarization
            speaker = "SPEAKER_00"

            if diarization_segments:
                chunk_mid = (start + end) / 2
                best_overlap = 0

                for seg in diarization_segments:
                    seg_start = seg["start"]
                    seg_end = seg["end"]

                    # Check overlap
                    overlap_start = max(start, seg_start)
                    overlap_end = min(end, seg_end)
                    overlap = max(0, overlap_end - overlap_start)

                    if overlap > best_overlap:
                        best_overlap = overlap
                        speaker = seg["speaker"]

                    # Also check if midpoint falls within segment
                    if seg_start <= chunk_mid <= seg_end:
                        speaker = seg["speaker"]
                        break

            speakers_set.add(speaker)

            segments.append(
                TranscriptionSegment(
                    speaker=speaker,
                    start=round(start, 2),
                    end=round(end, 2),
                    text=text,
                )
            )

        speakers = sorted(speakers_set) if speakers_set else ["SPEAKER_00"]
        return segments, speakers

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
        logger.info(f"[Kotoba] Starting transcription: {audio_path}")

        # Load pipeline
        pipe = self._load_pipeline()

        # Configure generation kwargs
        generate_kwargs = {
            "language": language,
            "task": "transcribe",
        }

        # Add prompt for filler preservation
        if filler_prompt and language == "ja":
            logger.info(f"[Kotoba] Using filler prompt: {filler_prompt[:50]}...")
            # kotoba-whisper supports prompt via generate_kwargs
            generate_kwargs["prompt_ids"] = pipe.tokenizer.get_prompt_ids(
                filler_prompt, return_tensors="pt"
            ).to(self.device)

        # Run transcription
        logger.info("[Kotoba] Running ASR...")
        result = pipe(
            audio_path,
            return_timestamps=True,
            generate_kwargs=generate_kwargs,
        )

        # Free VRAM
        del pipe
        torch.cuda.empty_cache()

        # Get duration from audio
        import torchaudio

        waveform, sample_rate = torchaudio.load(audio_path)
        duration = waveform.shape[1] / sample_rate

        # Run speaker diarization
        diarization_segments = self._run_diarization(
            audio_path, min_speakers, max_speakers
        )

        # Process chunks and assign speakers
        chunks = result.get("chunks", [])
        if not chunks:
            # Fallback to single segment
            text = result.get("text", "").strip()
            chunks = [{"text": text, "timestamp": (0, duration)}]

        segments, speakers = self._assign_speakers(chunks, diarization_segments)

        logger.info(
            f"[Kotoba] Transcription complete: {len(segments)} segments, "
            f"{len(speakers)} speakers, {duration:.1f}s"
        )

        return TranscriptionResult(
            segments=segments,
            speakers=speakers,
            duration_seconds=round(duration, 2),
            language=language,
        )
