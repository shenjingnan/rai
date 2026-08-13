import { ListMusic } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { DetectionEntry } from "@/hooks/useResults";

export function ResultsCard({ results }: { results: DetectionEntry[] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <ListMusic className="h-4 w-4 text-muted-foreground" />
          检测结果
        </CardTitle>
        <CardDescription>最近检测到的唤醒词</CardDescription>
      </CardHeader>
      <CardContent>
        {results.length === 0 ? (
          <p className="text-sm text-muted-foreground">尚未检测到唤醒词</p>
        ) : (
          <ul className="max-h-56 overflow-y-auto">
            {results.map((r) => (
              <li key={r.id} className="border-b py-1.5 text-sm last:border-b-0">
                [{r.at}] {r.keyword}（start={r.startTime.toFixed(2)}s）
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
