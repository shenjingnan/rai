import { ArrowLeft } from "lucide-react";
import { Link } from "react-router-dom";

/** 模型库页骨架：KWS/ASR/LLM/TTS 本地模型文件管理（本轮仅占位，后续实现）。 */
export function LibraryPage() {
  return (
    <div className="space-y-4">
      <Link
        to="/models"
        className="inline-flex items-center gap-1.5 text-sm text-text-secondary transition-colors hover:text-text-primary"
      >
        <ArrowLeft className="h-4 w-4" />
        模型与能力
      </Link>
      <h1 className="text-xl font-semibold tracking-tight text-text-primary">模型库</h1>
      <p className="text-sm text-muted-foreground">本地模型文件管理（后续实现）</p>
    </div>
  );
}
