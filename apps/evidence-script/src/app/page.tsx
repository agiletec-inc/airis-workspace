"use client";

import { useState } from "react";
import { UploadSection } from "@/components/upload-section";
import { TranscriptViewer } from "@/components/transcript-viewer";
import type { TranscriptionResult } from "@/types/transcript";

export default function Home() {
  const [result, setResult] = useState<TranscriptionResult | null>(null);

  return (
    <main className="min-h-screen p-8">
      <div className="max-w-6xl mx-auto space-y-8">
        <header className="text-center space-y-2">
          <h1 className="text-3xl font-bold">Evidence Script</h1>
          <p className="text-gray-600 dark:text-gray-400">
            裁判証拠用 文字起こしシステム
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-500">
            フィラー（えーと、あのー等）を含む完全な逐語録と話者分離
          </p>
        </header>

        <UploadSection onComplete={setResult} />
        <TranscriptViewer result={result ?? undefined} />
      </div>
    </main>
  );
}
