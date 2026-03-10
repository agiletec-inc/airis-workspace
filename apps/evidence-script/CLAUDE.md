# CLAUDE.md

This file provides guidance to Claude Code when working with the Evidence Script application.

## Project Overview

**Evidence Script** is a court-ready transcription system that produces verbatim transcripts with filler words (fillers like "um", "uh", Japanese "えーと", "あのー") and speaker diarization.

**Purpose**: Generate legally admissible transcripts for court evidence from audio/video recordings.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Next.js Frontend                          │
│  - Drag & drop upload UI                                    │
│  - Transcript viewer with speaker labels                    │
│  - Export to JSON/TXT                                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    API Route                                 │
│  /api/evidence/transcribe-v2                                │
│  - Validates request                                        │
│  - Forwards to GPU worker                                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 GPU Worker (Python)                          │
│  - FFmpeg: video → audio extraction                         │
│  - WhisperX: transcription + alignment + diarization        │
│  - Filler preservation via initial_prompt                   │
└─────────────────────────────────────────────────────────────┘
```

## Key Technologies

- **Frontend**: Next.js 15, React, Tailwind CSS, react-dropzone
- **Backend**: FastAPI (Python), WhisperX, Pyannote Audio
- **Storage**: Supabase Storage
- **GPU**: NVIDIA CUDA 12.4

## Development Commands

```bash
# Start GPU worker (requires NVIDIA GPU)
docker compose -f apps/evidence-script/compose.yaml up

# Start Next.js dev server (in workspace container)
docker compose exec workspace bash -c "cd apps/evidence-script && pnpm dev"
```

## Environment Variables

Required:
- `NEXT_PUBLIC_SUPABASE_URL` - Supabase project URL
- `NEXT_PUBLIC_SUPABASE_ANON_KEY` - Supabase anon key
- `SUPABASE_SERVICE_KEY` - Supabase service role key (for worker)
- `HF_TOKEN` - HuggingFace token (for Pyannote speaker diarization)

Optional:
- `EVIDENCE_WORKER_URL` - GPU worker URL (default: http://localhost:8001)
- `WHISPER_MODEL` - Whisper model size (default: large-v3)

## Key Implementation Details

### Filler Preservation

The transcription uses WhisperX with an `initial_prompt` containing common filler words to encourage their retention:

```python
FILLER_PROMPT_JA = (
    "えーと、あのー、うーん、えー、その、まあ、なんか、ちょっと、"
    "はい、いや、そうですね、あ、ああ、うん、ええ、おー、"
    "ですから、なので、つまり、要するに、"
    "えっと、あのですね、ちょっとですね"
)
```

### Speaker Diarization

Uses Pyannote Audio 3.3 via WhisperX integration. Requires HuggingFace token with access to:
- `pyannote/speaker-diarization-3.1`
- `pyannote/segmentation-3.0`

### Output Format

```json
{
  "metadata": {
    "file_name": "call.mp4",
    "duration_seconds": 180,
    "speakers": [
      { "id": "SPEAKER_00", "label": "話者A" },
      { "id": "SPEAKER_01", "label": "話者B" }
    ]
  },
  "transcript": [
    {
      "speaker": "話者A",
      "speaker_id": "SPEAKER_00",
      "start": 0.0,
      "end": 5.2,
      "text": "えーと、本日はお電話いただきありがとうございます。"
    }
  ]
}
```

## Testing

```bash
# Test local file transcription (requires GPU worker running)
curl -X POST http://localhost:8001/transcribe/local \
  -H "Content-Type: application/json" \
  -d '{"file_path": "/data/test.wav", "language": "ja"}'
```

## Directory Structure

```
apps/evidence-script/
├── compose.yaml              # GPU worker service
├── Dockerfile                 # Next.js container
├── worker/
│   ├── Dockerfile            # GPU worker container
│   ├── main.py               # FastAPI server
│   ├── transcriber.py        # WhisperX integration
│   └── requirements.txt      # Python dependencies
└── src/
    ├── app/
    │   ├── api/evidence/     # API routes
    │   ├── layout.tsx
    │   └── page.tsx
    ├── components/
    │   ├── upload-section.tsx
    │   └── transcript-viewer.tsx
    ├── lib/
    │   ├── supabase/
    │   └── utils.ts
    └── types/
        └── transcript.ts
```
