"use client";

import { useCallback, useState } from "react";
import { useDropzone } from "react-dropzone";
import { Upload, FileAudio, FileVideo, Loader2, CheckCircle, XCircle } from "lucide-react";
import { toast } from "sonner";
import { cn, formatFileSize } from "@/lib/utils";
import { createClient } from "@/lib/supabase/client";
import type { UploadedFile, TranscriptionResult } from "@/types/transcript";

const ACCEPTED_TYPES = {
  "audio/*": [".mp3", ".wav", ".m4a", ".aac", ".ogg", ".flac"],
  "video/*": [".mp4", ".mov", ".avi", ".mkv", ".webm"],
};

const MAX_FILE_SIZE = 500 * 1024 * 1024; // 500MB

interface UploadSectionProps {
  onComplete?: (result: TranscriptionResult) => void;
}

export function UploadSection({ onComplete }: UploadSectionProps) {
  const [files, setFiles] = useState<UploadedFile[]>([]);
  const [language, setLanguage] = useState("ja");

  const uploadFile = async (file: File): Promise<UploadedFile> => {
    const id = crypto.randomUUID();
    const uploadedFile: UploadedFile = {
      id,
      name: file.name,
      size: file.size,
      type: file.type,
      storage_path: "",
      status: "uploading",
      progress: 0,
    };

    setFiles((prev) => [...prev, uploadedFile]);

    try {
      // Upload to Supabase Storage
      const supabase = createClient();
      const storagePath = `evidence/${id}/${file.name}`;

      const { error: uploadError } = await supabase.storage
        .from("uploads")
        .upload(storagePath, file, {
          cacheControl: "3600",
          upsert: false,
        });

      if (uploadError) throw uploadError;

      setFiles((prev) =>
        prev.map((f) =>
          f.id === id
            ? { ...f, storage_path: `uploads/${storagePath}`, status: "uploaded", progress: 100 }
            : f
        )
      );

      // Start transcription
      setFiles((prev) =>
        prev.map((f) => (f.id === id ? { ...f, status: "processing" } : f))
      );

      const response = await fetch("/api/evidence/transcribe-v2", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          storage_path: `uploads/${storagePath}`,
          language,
        }),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Transcription failed");
      }

      const result: TranscriptionResult = await response.json();

      setFiles((prev) =>
        prev.map((f) =>
          f.id === id
            ? {
                ...f,
                status: result.status === "error" ? "error" : "completed",
                result,
                error: result.error,
              }
            : f
        )
      );

      if (result.status === "success") {
        toast.success(`${file.name} の文字起こしが完了しました`);
        onComplete?.(result);
      }

      return { ...uploadedFile, result };
    } catch (error) {
      const message = error instanceof Error ? error.message : "Upload failed";
      setFiles((prev) =>
        prev.map((f) =>
          f.id === id ? { ...f, status: "error", error: message } : f
        )
      );
      toast.error(`${file.name}: ${message}`);
      throw error;
    }
  };

  const onDrop = useCallback(
    async (acceptedFiles: File[]) => {
      for (const file of acceptedFiles) {
        if (file.size > MAX_FILE_SIZE) {
          toast.error(`${file.name} is too large (max 500MB)`);
          continue;
        }
        await uploadFile(file);
      }
    },
    [language]
  );

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    onDrop,
    accept: ACCEPTED_TYPES,
    maxSize: MAX_FILE_SIZE,
  });

  const getStatusIcon = (status: UploadedFile["status"]) => {
    switch (status) {
      case "uploading":
      case "processing":
        return <Loader2 className="w-5 h-5 animate-spin text-blue-500" />;
      case "completed":
        return <CheckCircle className="w-5 h-5 text-green-500" />;
      case "error":
        return <XCircle className="w-5 h-5 text-red-500" />;
      default:
        return null;
    }
  };

  const getStatusText = (status: UploadedFile["status"]) => {
    switch (status) {
      case "uploading":
        return "アップロード中...";
      case "uploaded":
        return "アップロード完了";
      case "processing":
        return "文字起こし中...";
      case "completed":
        return "完了";
      case "error":
        return "エラー";
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center gap-4">
        <label className="text-sm font-medium">言語:</label>
        <select
          value={language}
          onChange={(e) => setLanguage(e.target.value)}
          className="px-3 py-1.5 border rounded-md text-sm bg-white dark:bg-gray-800"
        >
          <option value="ja">日本語</option>
          <option value="en">English</option>
        </select>
      </div>

      <div
        {...getRootProps()}
        className={cn(
          "border-2 border-dashed rounded-xl p-12 text-center cursor-pointer transition-colors",
          isDragActive
            ? "border-blue-500 bg-blue-50 dark:bg-blue-950"
            : "border-gray-300 hover:border-gray-400 dark:border-gray-700"
        )}
      >
        <input {...getInputProps()} />
        <Upload className="w-12 h-12 mx-auto mb-4 text-gray-400" />
        {isDragActive ? (
          <p className="text-lg font-medium text-blue-600">
            ファイルをドロップしてください
          </p>
        ) : (
          <>
            <p className="text-lg font-medium mb-2">
              音声・動画ファイルをドラッグ＆ドロップ
            </p>
            <p className="text-sm text-gray-500">
              または クリックしてファイルを選択
            </p>
            <p className="text-xs text-gray-400 mt-2">
              対応形式: MP3, WAV, M4A, MP4, MOV など (最大500MB)
            </p>
          </>
        )}
      </div>

      {files.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-sm font-medium">アップロードファイル</h3>
          <div className="space-y-2">
            {files.map((file) => (
              <div
                key={file.id}
                className="flex items-center gap-3 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg"
              >
                {file.type.startsWith("video") ? (
                  <FileVideo className="w-5 h-5 text-purple-500" />
                ) : (
                  <FileAudio className="w-5 h-5 text-blue-500" />
                )}
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{file.name}</p>
                  <p className="text-xs text-gray-500">
                    {formatFileSize(file.size)} · {getStatusText(file.status)}
                    {file.error && (
                      <span className="text-red-500 ml-2">{file.error}</span>
                    )}
                  </p>
                </div>
                {getStatusIcon(file.status)}
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
