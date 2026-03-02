"""
Evidence Script Worker - FastAPI Server

Endpoints:
- POST /transcribe: Transcribe audio/video file
- GET /health: Health check
"""

import logging
import os
import tempfile
from dataclasses import asdict
from pathlib import Path
from typing import Literal

import httpx
from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
from pydantic_settings import BaseSettings

from backends import get_backend
from transcriber import EvidenceTranscriber

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)


class Settings(BaseSettings):
    """Application settings from environment variables."""

    hf_token: str = Field(default="", alias="HF_TOKEN")
    supabase_url: str = Field(default="", alias="SUPABASE_URL")
    supabase_service_key: str = Field(default="", alias="SUPABASE_SERVICE_KEY")
    whisper_model: str = Field(default="large-v3", alias="WHISPER_MODEL")
    device: str = Field(default="cuda", alias="DEVICE")
    compute_type: str = Field(default="float16", alias="COMPUTE_TYPE")
    stt_backend: str = Field(default="whisperx", alias="STT_BACKEND")

    class Config:
        env_file = ".env"


settings = Settings()

# Initialize FastAPI app
app = FastAPI(
    title="Evidence Script Worker",
    description="Court-ready transcription service with filler preservation",
    version="1.0.0",
)

# CORS for development
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Initialize transcriber (lazy loading)
_transcriber: EvidenceTranscriber | None = None


def get_transcriber() -> EvidenceTranscriber:
    """Get or create transcriber instance."""
    global _transcriber
    if _transcriber is None:
        # Backend selection respects STT_BACKEND env var via get_backend()
        backend = get_backend(settings.stt_backend)
        _transcriber = EvidenceTranscriber(backend=backend)
    return _transcriber


# Request/Response models
class TranscribeRequest(BaseModel):
    """Request to transcribe a file."""

    storage_path: str = Field(
        ...,
        description="Supabase Storage path (bucket/path/to/file)",
    )
    language: str = Field(
        default="ja",
        description="Language code (ja, en)",
    )
    min_speakers: int | None = Field(
        default=None,
        description="Minimum expected speakers",
    )
    max_speakers: int | None = Field(
        default=None,
        description="Maximum expected speakers",
    )
    callback_url: str | None = Field(
        default=None,
        description="Webhook URL for async notification",
    )


class TranscriptSegmentResponse(BaseModel):
    """Single transcript segment."""

    speaker: str
    start: float
    end: float
    text: str


class TranscribeResponse(BaseModel):
    """Transcription result."""

    status: Literal["success", "processing", "error"]
    file_name: str
    duration_seconds: float
    language: str
    speakers: list[str]
    segments: list[TranscriptSegmentResponse]
    error: str | None = None


class HealthResponse(BaseModel):
    """Health check response."""

    status: str
    gpu_available: bool
    model_loaded: bool
    stt_backend: str


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    import torch

    gpu_available = torch.cuda.is_available()
    model_loaded = _transcriber is not None

    return HealthResponse(
        status="healthy" if gpu_available else "degraded",
        gpu_available=gpu_available,
        model_loaded=model_loaded,
        stt_backend=settings.stt_backend,
    )


async def download_from_supabase(storage_path: str, local_path: str) -> None:
    """Download file from Supabase Storage."""
    if not settings.supabase_url or not settings.supabase_service_key:
        raise HTTPException(
            status_code=500,
            detail="Supabase credentials not configured",
        )

    # Parse bucket and path
    parts = storage_path.split("/", 1)
    if len(parts) != 2:
        raise HTTPException(
            status_code=400,
            detail="Invalid storage_path format. Use: bucket/path/to/file",
        )

    bucket, file_path = parts

    # Download from Supabase Storage
    url = f"{settings.supabase_url}/storage/v1/object/{bucket}/{file_path}"
    headers = {
        "Authorization": f"Bearer {settings.supabase_service_key}",
        "apikey": settings.supabase_service_key,
    }

    async with httpx.AsyncClient() as client:
        response = await client.get(url, headers=headers, timeout=300)

        if response.status_code != 200:
            raise HTTPException(
                status_code=response.status_code,
                detail=f"Failed to download file: {response.text}",
            )

        Path(local_path).parent.mkdir(parents=True, exist_ok=True)
        with open(local_path, "wb") as f:
            f.write(response.content)


async def send_callback(callback_url: str, result: dict) -> None:
    """Send result to callback URL."""
    try:
        async with httpx.AsyncClient() as client:
            await client.post(callback_url, json=result, timeout=30)
    except Exception as e:
        logger.error(f"Failed to send callback: {e}")


def process_transcription(
    storage_path: str,
    language: str,
    min_speakers: int | None,
    max_speakers: int | None,
    callback_url: str | None,
) -> TranscribeResponse:
    """Process transcription (sync for background task)."""
    import asyncio

    file_name = Path(storage_path).name

    try:
        # Download file
        with tempfile.TemporaryDirectory() as tmpdir:
            local_path = os.path.join(tmpdir, file_name)

            # Run async download in sync context
            asyncio.run(download_from_supabase(storage_path, local_path))

            # Determine if video or audio
            video_extensions = {".mp4", ".mov", ".avi", ".mkv", ".webm"}
            is_video = Path(file_name).suffix.lower() in video_extensions

            # Transcribe
            transcriber = get_transcriber()

            if is_video:
                result = transcriber.transcribe_video(
                    local_path,
                    language=language,
                    min_speakers=min_speakers,
                    max_speakers=max_speakers,
                )
            else:
                result = transcriber.transcribe(
                    local_path,
                    language=language,
                    min_speakers=min_speakers,
                    max_speakers=max_speakers,
                )

        response = TranscribeResponse(
            status="success",
            file_name=file_name,
            duration_seconds=result.duration_seconds,
            language=result.language,
            speakers=result.speakers,
            segments=[
                TranscriptSegmentResponse(
                    speaker=seg.speaker,
                    start=seg.start,
                    end=seg.end,
                    text=seg.text,
                )
                for seg in result.segments
            ],
        )

        # Send callback if specified
        if callback_url:
            asyncio.run(send_callback(callback_url, response.model_dump()))

        return response

    except Exception as e:
        logger.exception(f"Transcription failed: {e}")
        error_response = TranscribeResponse(
            status="error",
            file_name=file_name,
            duration_seconds=0,
            language=language,
            speakers=[],
            segments=[],
            error=str(e),
        )

        if callback_url:
            asyncio.run(send_callback(callback_url, error_response.model_dump()))

        return error_response


@app.post("/transcribe", response_model=TranscribeResponse)
async def transcribe(
    request: TranscribeRequest,
    background_tasks: BackgroundTasks,
):
    """
    Transcribe audio/video file from Supabase Storage.

    If callback_url is provided, returns immediately with status="processing"
    and sends result to callback URL when complete.
    """
    file_name = Path(request.storage_path).name

    if request.callback_url:
        # Async processing with callback
        background_tasks.add_task(
            process_transcription,
            request.storage_path,
            request.language,
            request.min_speakers,
            request.max_speakers,
            request.callback_url,
        )

        return TranscribeResponse(
            status="processing",
            file_name=file_name,
            duration_seconds=0,
            language=request.language,
            speakers=[],
            segments=[],
        )

    # Sync processing (blocking)
    return process_transcription(
        request.storage_path,
        request.language,
        request.min_speakers,
        request.max_speakers,
        None,
    )


@app.post("/transcribe/local")
async def transcribe_local(
    file_path: str,
    language: str = "ja",
    min_speakers: int | None = None,
    max_speakers: int | None = None,
):
    """
    Transcribe a local file (for testing).

    Args:
        file_path: Absolute path to local audio/video file
        language: Language code
        min_speakers: Minimum speakers
        max_speakers: Maximum speakers
    """
    if not Path(file_path).exists():
        raise HTTPException(status_code=404, detail="File not found")

    file_name = Path(file_path).name
    video_extensions = {".mp4", ".mov", ".avi", ".mkv", ".webm"}
    is_video = Path(file_name).suffix.lower() in video_extensions

    try:
        transcriber = get_transcriber()

        if is_video:
            result = transcriber.transcribe_video(
                file_path,
                language=language,
                min_speakers=min_speakers,
                max_speakers=max_speakers,
            )
        else:
            result = transcriber.transcribe(
                file_path,
                language=language,
                min_speakers=min_speakers,
                max_speakers=max_speakers,
            )

        return TranscribeResponse(
            status="success",
            file_name=file_name,
            duration_seconds=result.duration_seconds,
            language=result.language,
            speakers=result.speakers,
            segments=[
                TranscriptSegmentResponse(
                    speaker=seg.speaker,
                    start=seg.start,
                    end=seg.end,
                    text=seg.text,
                )
                for seg in result.segments
            ],
        )

    except Exception as e:
        logger.exception(f"Transcription failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
