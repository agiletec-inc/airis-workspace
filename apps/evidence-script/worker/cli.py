#!/usr/bin/env python3
"""
Evidence Transcriber CLI

Compare STT backends (whisperx vs kotoba) on the same audio file.

Usage:
    python cli.py --input test.wav --backend whisperx --output whisperx.json
    python cli.py --input test.wav --backend kotoba --output kotoba.json
"""

import argparse
import json
import logging
import sys
from dataclasses import asdict
from pathlib import Path

from backends import get_backend
from transcriber import EvidenceTranscriber

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Transcribe audio with selectable STT backend",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Transcribe with WhisperX (default)
    python cli.py --input recording.wav --speakers 2

    # Transcribe with Kotoba backend
    python cli.py --input recording.wav --backend kotoba --speakers 2

    # Compare both backends
    python cli.py --input test.wav --backend whisperx --output whisperx.json
    python cli.py --input test.wav --backend kotoba --output kotoba.json
    diff whisperx.json kotoba.json
        """,
    )

    parser.add_argument(
        "--input", "-i",
        required=True,
        help="Path to audio file (WAV/MP3/etc)",
    )
    parser.add_argument(
        "--backend", "-b",
        choices=["whisperx", "kotoba"],
        default="whisperx",
        help="STT backend to use (default: whisperx)",
    )
    parser.add_argument(
        "--language", "-l",
        default="ja",
        help="Language code (default: ja)",
    )
    parser.add_argument(
        "--speakers", "-s",
        type=int,
        default=None,
        help="Maximum number of speakers (optional)",
    )
    parser.add_argument(
        "--min-speakers",
        type=int,
        default=None,
        help="Minimum number of speakers (optional)",
    )
    parser.add_argument(
        "--output", "-o",
        default=None,
        help="Output JSON file (default: stdout)",
    )
    parser.add_argument(
        "--pretty", "-p",
        action="store_true",
        help="Pretty-print JSON output",
    )

    args = parser.parse_args()

    # Validate input file
    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: Input file not found: {args.input}", file=sys.stderr)
        return 1

    # Get backend
    backend = get_backend(args.backend)
    print(f"Using backend: {args.backend}", file=sys.stderr)

    # Create transcriber
    transcriber = EvidenceTranscriber(backend=backend)

    # Run transcription
    result = transcriber.transcribe(
        audio_path=str(input_path),
        language=args.language,
        min_speakers=args.min_speakers,
        max_speakers=args.speakers,
    )

    # Convert to dict for JSON serialization
    output = {
        "backend": args.backend,
        "input": str(input_path),
        "language": result.language,
        "duration_seconds": result.duration_seconds,
        "speakers": result.speakers,
        "segments": [asdict(seg) for seg in result.segments],
    }

    # Output
    json_kwargs = {"ensure_ascii": False}
    if args.pretty:
        json_kwargs["indent"] = 2

    json_output = json.dumps(output, **json_kwargs)

    if args.output:
        output_path = Path(args.output)
        output_path.write_text(json_output, encoding="utf-8")
        print(f"Output written to: {args.output}", file=sys.stderr)
    else:
        print(json_output)

    # Print summary
    print(
        f"\nSummary: {len(result.segments)} segments, "
        f"{len(result.speakers)} speakers, "
        f"{result.duration_seconds:.1f}s",
        file=sys.stderr,
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())
