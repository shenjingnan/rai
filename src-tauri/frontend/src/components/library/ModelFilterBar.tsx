import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { PARAMETER_OPTIONS, SORT_OPTIONS } from "@/lib/catalog/query";
import type { CatalogSort, ParameterRange } from "@/types/catalog";

interface ModelFilterBarProps {
  search: string;
  onSearch: (s: string) => void;
  language: string | null;
  onLanguage: (l: string | null) => void;
  parameters: ParameterRange | null;
  onParameters: (p: ParameterRange | null) => void;
  sort: CatalogSort;
  onSort: (s: CatalogSort) => void;
  showAll: boolean;
  onToggleShowAll: () => void;
}

const LANG_OPTIONS = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
  { value: "ja", label: "日语" },
  { value: "ko", label: "韩语" },
];

/** 搜索 + 筛选（语言/参数量/排序 + 两个显示开关）。 */
export function ModelFilterBar(props: ModelFilterBarProps) {
  return (
    <div className="rounded-[16px] bg-panel-background py-3 pl-0 pr-4">
      <div className="relative">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" />
        <Input
          value={props.search}
          onChange={(e) => props.onSearch(e.target.value)}
          placeholder="搜索模型名称、描述、标签或作者..."
          className="pl-9"
        />
      </div>
      <div className="mt-2.5 flex flex-wrap items-center gap-2">
        <Select
          value={props.language ?? "all"}
          onValueChange={(v) => props.onLanguage(v === "all" ? null : v)}
        >
          <SelectTrigger className="h-9 w-auto min-w-28">
            <SelectValue placeholder="全部语言" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部语言</SelectItem>
            {LANG_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          value={props.parameters ?? "all"}
          onValueChange={(v) => props.onParameters(v === "all" ? null : (v as ParameterRange))}
        >
          <SelectTrigger className="h-9 w-auto min-w-28">
            <SelectValue placeholder="参数量" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部参数量</SelectItem>
            {PARAMETER_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={props.sort} onValueChange={(v) => props.onSort(v as CatalogSort)}>
          <SelectTrigger className="h-9 w-auto min-w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SORT_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <label className="flex cursor-pointer items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={props.showAll}
              onChange={props.onToggleShowAll}
              className="h-3.5 w-3.5 accent-blue-600"
            />
            显示全部模型
          </label>
        </div>
      </div>
    </div>
  );
}
