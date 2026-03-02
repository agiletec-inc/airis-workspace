# Evidence Script - GPU Worker for Court-Ready Transcription
# Includes: FFmpeg (NVENC), faster-whisper, WhisperX, Pyannote, Kotoba-Whisper

FROM nvidia/cuda:12.4-runtime-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV PYTHONUNBUFFERED=1

# System dependencies + FFmpeg with NVIDIA support
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    python3 \
    python3-pip \
    python3-venv \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash worker && \
    mkdir -p /app /data && \
    chown -R worker:worker /app /data

WORKDIR /app
USER worker

# Create virtual environment
RUN python3 -m venv /app/venv
ENV PATH="/app/venv/bin:$PATH"

# Install Python dependencies
# WhisperX includes: faster-whisper, pyannote-audio, transformers
# Additional deps for kotoba-whisper: accelerate
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir \
    git+https://github.com/m-bain/whisperX.git \
    transformers>=4.40.0 \
    accelerate>=0.27.0 \
    pyannote.audio>=3.3.0 \
    torch>=2.1.0 \
    torchaudio>=2.1.0 \
    fastapi \
    uvicorn[standard] \
    python-multipart \
    httpx \
    pydantic \
    pydantic-settings

# Copy application code
COPY --chown=worker:worker . /app/

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# Environment variables for STT backend selection
# STT_BACKEND: whisperx (default) or kotoba
ENV STT_BACKEND=whisperx
ENV DEVICE=cuda
ENV COMPUTE_TYPE=float16
ENV WHISPER_MODEL=large-v3

EXPOSE 8000

CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
