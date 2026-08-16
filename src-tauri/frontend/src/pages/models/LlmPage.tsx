import { ArrowLeft } from "lucide-react";
import { Link } from "react-router-dom";
import { LlmCard } from "@/components/LlmCard";

export function LlmPage() {
  return (
    <div className="space-y-4">
      <Link
        to="/models"
        className="inline-flex items-center gap-1.5 text-sm text-text-secondary transition-colors hover:text-text-primary"
      >
        <ArrowLeft className="h-4 w-4" />
        模型与能力
      </Link>
      <h1 className="text-xl font-semibold tracking-tight text-text-primary">本地大模型（LLM）</h1>
      <LlmCard />
    </div>
  );
}
