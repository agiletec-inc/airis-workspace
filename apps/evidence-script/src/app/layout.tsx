import type { Metadata } from "next";
import { Toaster } from "sonner";
import "./globals.css";

export const metadata: Metadata = {
  title: "Evidence Script - 裁判証拠用文字起こし",
  description:
    "音声・動画ファイルから、フィラーを含む完全な逐語録を生成し、話者分離を行って裁判所提出用の証拠資料を作成します。",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ja">
      <body className="antialiased">
        {children}
        <Toaster position="top-right" richColors />
      </body>
    </html>
  );
}
