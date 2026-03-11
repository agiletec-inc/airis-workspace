import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";

const WORKER_URL = process.env.EVIDENCE_WORKER_URL || "http://localhost:8001";

const requestSchema = z.object({
  storage_path: z.string().min(1, "storage_path is required"),
  language: z.enum(["ja", "en"]).default("ja"),
  min_speakers: z.number().int().positive().optional(),
  max_speakers: z.number().int().positive().optional(),
});

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const parsed = requestSchema.safeParse(body);

    if (!parsed.success) {
      return NextResponse.json(
        { error: "Invalid request", details: parsed.error.flatten() },
        { status: 400 }
      );
    }

    const { storage_path, language, min_speakers, max_speakers } = parsed.data;

    // Forward request to GPU worker
    const workerResponse = await fetch(`${WORKER_URL}/transcribe`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        storage_path,
        language,
        min_speakers,
        max_speakers,
      }),
    });

    if (!workerResponse.ok) {
      const error = await workerResponse.json().catch(() => ({
        detail: "Worker request failed",
      }));
      return NextResponse.json(
        { error: error.detail || "Transcription failed" },
        { status: workerResponse.status }
      );
    }

    const result = await workerResponse.json();
    return NextResponse.json(result);
  } catch (error) {
    console.error("Transcription API error:", error);

    if (error instanceof Error && error.message.includes("fetch")) {
      return NextResponse.json(
        {
          error: "Worker service unavailable",
          details: "GPU worker is not running. Start with: docker compose -f apps/evidence-script/compose.yaml up",
        },
        { status: 503 }
      );
    }

    return NextResponse.json(
      { error: "Internal server error" },
      { status: 500 }
    );
  }
}
