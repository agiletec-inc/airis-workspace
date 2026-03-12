"use client";

import { useState } from "react";
import { Copy, Download, Edit2, Check, Users } from "lucide-react";
import { toast } from "sonner";
import { cn, formatTimestamp, formatDuration, getSpeakerColor } from "@/lib/utils";
import type { TranscriptionResult, TranscriptSegment, SpeakerLabel } from "@/types/transcript";

interface TranscriptViewerProps {
  result?: TranscriptionResult;
}

export function TranscriptViewer({ result }: TranscriptViewerProps) {
  const [speakerLabels, setSpeakerLabels] = useState<Record<string, string>>({});
  const [editingSpeaker, setEditingSpeaker] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  // Demo data for development
  const demoResult: TranscriptionResult = {
    status: "success",
    file_name: "call_20240301.mp4",
    duration_seconds: 125.5,
    language: "ja",
    speakers: ["SPEAKER_00", "SPEAKER_01"],
    segments: [
      {
        speaker: "SPEAKER_00",
        start: 0.0,
        end: 4.2,
        text: "えーと、本日はお電話いただきありがとうございます。",
      },
      {
        speaker: "SPEAKER_01",
        start: 4.8,
        end: 9.5,
        text: "はい、あのー、ちょっとお聞きしたいことがありまして。",
      },
      {
        speaker: "SPEAKER_00",
        start: 10.0,
        end: 12.3,
        text: "はい、どのようなご用件でしょうか。",
      },
      {
        speaker: "SPEAKER_01",
        start: 13.0,
        end: 22.5,
        text: "えっと、先日の契約の件なんですけど、まあ、その、ちょっと確認したいことがあって。",
      },
      {
        speaker: "SPEAKER_00",
        start: 23.0,
        end: 27.8,
        text: "承知いたしました。契約内容についてですね。少々お待ちください。",
      },
    ],
  };

  const data = result || demoResult;

  const getSpeakerLabel = (speakerId: string, index: number): string => {
    return speakerLabels[speakerId] || `話者${String.fromCharCode(65 + index)}`;
  };

  const handleEditSpeaker = (speakerId: string) => {
    setEditingSpeaker(speakerId);
    setEditValue(speakerLabels[speakerId] || "");
  };

  const handleSaveSpeaker = (speakerId: string) => {
    if (editValue.trim()) {
      setSpeakerLabels((prev) => ({ ...prev, [speakerId]: editValue.trim() }));
    }
    setEditingSpeaker(null);
    setEditValue("");
  };

  const handleCopyText = () => {
    const text = data.segments
      .map((seg) => {
        const label = getSpeakerLabel(
          seg.speaker,
          data.speakers.indexOf(seg.speaker)
        );
        return `${label}: ${seg.text}`;
      })
      .join("\n");

    navigator.clipboard.writeText(text);
    toast.success("テキストをコピーしました");
  };

  const handleDownloadJSON = () => {
    const output = {
      metadata: {
        file_name: data.file_name,
        duration_seconds: data.duration_seconds,
        speakers: data.speakers.map((id, i) => ({
          id,
          label: getSpeakerLabel(id, i),
        })),
        session_type: "transcription",
      },
      transcript: data.segments.map((seg) => ({
        speaker: getSpeakerLabel(
          seg.speaker,
          data.speakers.indexOf(seg.speaker)
        ),
        speaker_id: seg.speaker,
        start: seg.start,
        end: seg.end,
        text: seg.text,
      })),
    };

    const blob = new Blob([JSON.stringify(output, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${data.file_name.replace(/\.[^.]+$/, "")}_transcript.json`;
    a.click();
    URL.revokeObjectURL(url);

    toast.success("JSONファイルをダウンロードしました");
  };

  const handleDownloadText = () => {
    const lines = [
      `# 文字起こし: ${data.file_name}`,
      `# 録音時間: ${formatDuration(data.duration_seconds)}`,
      `# 話者数: ${data.speakers.length}`,
      "",
      ...data.segments.map((seg) => {
        const label = getSpeakerLabel(
          seg.speaker,
          data.speakers.indexOf(seg.speaker)
        );
        const time = formatTimestamp(seg.start);
        return `[${time}] ${label}: ${seg.text}`;
      }),
    ];

    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${data.file_name.replace(/\.[^.]+$/, "")}_transcript.txt`;
    a.click();
    URL.revokeObjectURL(url);

    toast.success("テキストファイルをダウンロードしました");
  };

  if (!data.segments.length) {
    return (
      <section className="text-center py-12 text-gray-500">
        <p>ファイルをアップロードすると、ここに文字起こし結果が表示されます</p>
      </section>
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">文字起こし結果</h2>
          <p className="text-sm text-gray-500">
            {data.file_name} · {formatDuration(data.duration_seconds)} ·{" "}
            {data.speakers.length}人の話者
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleCopyText}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border rounded-md hover:bg-gray-50 dark:hover:bg-gray-800"
          >
            <Copy className="w-4 h-4" />
            コピー
          </button>
          <button
            onClick={handleDownloadText}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm border rounded-md hover:bg-gray-50 dark:hover:bg-gray-800"
          >
            <Download className="w-4 h-4" />
            TXT
          </button>
          <button
            onClick={handleDownloadJSON}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700"
          >
            <Download className="w-4 h-4" />
            JSON
          </button>
        </div>
      </div>

      {/* Speaker Labels */}
      <div className="flex items-center gap-4 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg">
        <Users className="w-5 h-5 text-gray-500" />
        <div className="flex flex-wrap gap-2">
          {data.speakers.map((speakerId, index) => (
            <div
              key={speakerId}
              className="flex items-center gap-1.5 px-2 py-1 bg-white dark:bg-gray-700 rounded border"
            >
              <div
                className="w-3 h-3 rounded-full"
                style={{ backgroundColor: getSpeakerColor(index) }}
              />
              {editingSpeaker === speakerId ? (
                <div className="flex items-center gap-1">
                  <input
                    type="text"
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSaveSpeaker(speakerId);
                      if (e.key === "Escape") setEditingSpeaker(null);
                    }}
                    className="w-20 px-1 text-sm border rounded"
                    autoFocus
                  />
                  <button
                    onClick={() => handleSaveSpeaker(speakerId)}
                    className="p-0.5 hover:bg-gray-100 rounded"
                  >
                    <Check className="w-3.5 h-3.5 text-green-600" />
                  </button>
                </div>
              ) : (
                <>
                  <span className="text-sm font-medium">
                    {getSpeakerLabel(speakerId, index)}
                  </span>
                  <button
                    onClick={() => handleEditSpeaker(speakerId)}
                    className="p-0.5 hover:bg-gray-100 dark:hover:bg-gray-600 rounded"
                  >
                    <Edit2 className="w-3 h-3 text-gray-400" />
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Transcript */}
      <div className="border rounded-lg divide-y">
        {data.segments.map((segment, index) => {
          const speakerIndex = data.speakers.indexOf(segment.speaker);
          const label = getSpeakerLabel(segment.speaker, speakerIndex);
          const color = getSpeakerColor(speakerIndex);

          return (
            <div key={index} className="flex p-3 gap-3 hover:bg-gray-50 dark:hover:bg-gray-800/50">
              <div className="flex-shrink-0 w-20 text-xs text-gray-400 font-mono pt-0.5">
                {formatTimestamp(segment.start)}
              </div>
              <div className="flex-shrink-0">
                <span
                  className="inline-flex items-center gap-1.5 px-2 py-0.5 text-xs font-medium rounded-full text-white"
                  style={{ backgroundColor: color }}
                >
                  {label}
                </span>
              </div>
              <div className="flex-1 text-sm leading-relaxed">{segment.text}</div>
            </div>
          );
        })}
      </div>

      <div className="text-xs text-gray-400 text-center">
        ※ フィラー（えーと、あのー等）を含む完全な逐語録です
      </div>
    </section>
  );
}
