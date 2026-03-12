/**
 * Evidence Script - Transcript Types
 */

export interface TranscriptSegment {
  speaker: string;
  start: number;
  end: number;
  text: string;
}

export interface TranscriptionResult {
  status: "success" | "processing" | "error";
  file_name: string;
  duration_seconds: number;
  language: string;
  speakers: string[];
  segments: TranscriptSegment[];
  error?: string;
}

export interface TranscribeRequest {
  storage_path: string;
  language?: string;
  min_speakers?: number;
  max_speakers?: number;
  callback_url?: string;
}

export interface UploadedFile {
  id: string;
  name: string;
  size: number;
  type: string;
  storage_path: string;
  status: "uploading" | "uploaded" | "processing" | "completed" | "error";
  progress: number;
  result?: TranscriptionResult;
  error?: string;
}

export interface SpeakerLabel {
  id: string;
  name: string;
  color: string;
}
