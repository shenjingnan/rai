import { Database, Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { LibraryBulkBar } from "@/components/library/LibraryBulkBar";
import { LibraryCard } from "@/components/library/LibraryCard";
import {
  AddLocalModelDialog,
  ModelConfirmDialog,
  ModelDetailDialog,
} from "@/components/library/LibraryDialogs";
import { SystemResourcesCard } from "@/components/library/LibrarySidebar";
import { MODEL_TYPE_SHORT, TYPE_META } from "@/components/library/libraryMeta";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ModelCardSkeleton } from "@/components/ui/skeleton";
import { useModelLibrary } from "@/hooks/useModelLibrary";
import { api } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import type { LibraryModel, ModelType } from "@/types/modelLibrary";

type TypeFilter = ModelType | "all";
type LangFilter = "all" | "zh" | "en" | "multilingual";
type InstallFilter = "all" | "installed" | "not_installed" | "current" | "invalid";
type SortKey = "recommended" | "size" | "name" | "recent";

const TYPE_TABS: { value: TypeFilter; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "llm", label: "LLM" },
  { value: "asr", label: "ASR" },
  { value: "tts", label: "TTS" },
  { value: "kws", label: "KWS" },
];

const LANG_OPTIONS: { value: LangFilter; label: string }[] = [
  { value: "all", label: "全部语言" },
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
  { value: "multilingual", label: "多语言" },
];

const INSTALL_OPTIONS: { value: InstallFilter; label: string }[] = [
  { value: "all", label: "全部状态" },
  { value: "installed", label: "已安装" },
  { value: "not_installed", label: "未安装" },
  { value: "current", label: "当前使用" },
  { value: "invalid", label: "异常" },
];

const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "recommended", label: "排序：推荐" },
  { value: "size", label: "模型大小" },
  { value: "name", label: "名称" },
  { value: "recent", label: "最近安装" },
];

function matchesInstall(m: LibraryModel, f: InstallFilter): boolean {
  switch (f) {
    case "installed":
      return m.installState === "installed";
    case "not_installed":
      return m.installState === "not_installed";
    case "current":
      return m.current;
    case "invalid":
      return m.installState === "invalid";
    default:
      return true;
  }
}

function matchesLang(m: LibraryModel, f: LangFilter): boolean {
  if (f === "all") return true;
  if (f === "multilingual") return m.languages.includes("multilingual") || m.languages.length > 1;
  return m.languages.includes(f);
}

/** 模型库主页：发现 / 下载 / 管理 / 使用 AI 模型。 */
export function LibraryPage() {
  const lib = useModelLibrary();

  const [search, setSearch] = useState("");
  const [activeType, setActiveType] = useState<TypeFilter>("all");
  const [langFilter, setLangFilter] = useState<LangFilter>("all");
  const [installFilter, setInstallFilter] = useState<InstallFilter>("all");
  const [sort, setSort] = useState<SortKey>("recommended");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const [confirmModel, setConfirmModel] = useState<LibraryModel | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [detailModel, setDetailModel] = useState<LibraryModel | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [addRegistryId, setAddRegistryId] = useState<string | null>(null);

  const models = lib.models ?? [];

  const byType = useMemo(
    () => (activeType === "all" ? models : models.filter((m) => m.modelType === activeType)),
    [models, activeType],
  );

  const bySearch = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return byType;
    return byType.filter((m) =>
      [m.name, m.displayName, m.description, ...m.tags].join(" ").toLowerCase().includes(q),
    );
  }, [byType, search]);

  const byInstall = useMemo(
    () => bySearch.filter((m) => matchesInstall(m, installFilter)),
    [bySearch, installFilter],
  );

  const byLang = useMemo(
    () => byInstall.filter((m) => matchesLang(m, langFilter)),
    [byInstall, langFilter],
  );

  const filtered = useMemo(() => {
    const list = byLang;
    switch (sort) {
      case "size":
        return [...list].sort((a, b) => (b.sizeBytes ?? 0) - (a.sizeBytes ?? 0));
      case "name":
        return [...list].sort((a, b) => a.displayName.localeCompare(b.displayName));
      case "recent":
        return [...list].sort((a, b) => (b.installedAt ?? "").localeCompare(a.installedAt ?? ""));
      default:
        // 推荐：current → installed → registry 原始顺序（Array.sort 稳定）
        return [...list].sort(
          (a, b) =>
            Number(b.current) - Number(a.current) ||
            Number(b.installState === "installed") - Number(a.installState === "installed"),
        );
    }
  }, [byLang, sort]);

  const clearFilters = () => {
    setSearch("");
    setActiveType("all");
    setLangFilter("all");
    setInstallFilter("all");
    setSort("recommended");
  };

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAllFiltered = () => {
    const eligible = filtered.filter(
      (m) => m.ownership === "managed" && m.installState === "installed" && !m.current,
    );
    setSelectedIds(new Set(eligible.map((m) => m.id)));
  };

  const batchDelete = async () => {
    const eligible = filtered.filter(
      (m) =>
        selectedIds.has(m.id) &&
        m.ownership === "managed" &&
        m.installState === "installed" &&
        !m.current,
    );
    for (const m of eligible) {
      await lib.remove(m.id);
    }
    setSelectedIds(new Set());
  };

  const openAddTop = () => {
    setAddRegistryId(null);
    setAddOpen(true);
  };
  const openImport = (model: LibraryModel) => {
    setAddRegistryId(model.id);
    setAddOpen(true);
  };
  const openConfirm = (model: LibraryModel) => {
    setConfirmModel(model);
    setConfirmOpen(true);
  };
  const confirmDelete = async (model: LibraryModel) => {
    setConfirmOpen(false);
    setConfirmModel(null);
    await lib.remove(model.id);
  };

  return (
    <div className="flex flex-col gap-3">
      {/* 顶部 */}
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-text-primary">模型库</h1>
          <p className="mt-0.5 text-sm text-text-secondary">发现、下载和管理 AI 模型</p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            className="shadow-none"
            onClick={() => lib.refresh()}
            disabled={lib.refreshing}
          >
            <RefreshCw className={cn("h-4 w-4", lib.refreshing && "animate-spin")} />
            刷新列表
          </Button>
          <Button size="sm" onClick={openAddTop}>
            <Plus className="h-4 w-4" />
            添加本地模型
          </Button>
        </div>
      </header>

      {/* 搜索 + 筛选工具栏 */}
      <div className="rounded-[16px] border border-panel-border bg-panel-background px-4 py-3">
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索模型名称、描述或标签..."
            className="pl-9"
          />
        </div>
        <div className="mt-2.5 flex flex-wrap items-center gap-2">
          <Select value={activeType} onValueChange={(v) => setActiveType(v as TypeFilter)}>
            <SelectTrigger className="h-9 w-auto min-w-28">
              <SelectValue placeholder="全部类型" />
            </SelectTrigger>
            <SelectContent>
              {TYPE_TABS.map((t) => (
                <SelectItem key={t.value} value={t.value}>
                  {t.value === "all" ? "全部类型" : MODEL_TYPE_SHORT[t.value as ModelType]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={langFilter} onValueChange={(v) => setLangFilter(v as LangFilter)}>
            <SelectTrigger className="h-9 w-auto min-w-28">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {LANG_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={installFilter} onValueChange={(v) => setInstallFilter(v as InstallFilter)}>
            <SelectTrigger className="h-9 w-auto min-w-28">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {INSTALL_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={sort} onValueChange={(v) => setSort(v as SortKey)}>
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
        </div>
      </div>

      {/* 类型快速 Tab */}
      <div className="flex flex-wrap items-center gap-1.5">
        {TYPE_TABS.map((t) => {
          const active = activeType === t.value;
          const Icon = t.value === "all" ? Database : TYPE_META[t.value as ModelType].icon;
          return (
            <button
              key={t.value}
              type="button"
              onClick={() => setActiveType(t.value)}
              className={cn(
                "inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-sm font-medium transition-colors",
                active
                  ? "bg-nav-active text-primary"
                  : "text-text-secondary hover:bg-nav-hover hover:text-text-primary",
              )}
            >
              <Icon className="h-4 w-4" />
              {t.label}
            </button>
          );
        })}
      </div>

      {/* 左右布局 */}
      <div className="grid grid-cols-[190px_minmax(0,1fr)] items-start gap-3">
        <aside className="space-y-4">
          <SystemResourcesCard
            resources={lib.resources}
            loading={lib.resourcesLoading}
            onRefresh={lib.refreshResources}
          />
        </aside>

        <div className="space-y-2">
          {lib.error && (
            <Alert variant="destructive">
              <AlertDescription className="whitespace-pre-wrap">
                {`模型库加载失败\n\n${lib.error}`}
              </AlertDescription>
            </Alert>
          )}

          {lib.loading && lib.models === null ? (
            <div className="space-y-2">
              {[0, 1, 2, 3].map((i) => (
                <ModelCardSkeleton key={i} />
              ))}
            </div>
          ) : filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 rounded-[16px] border border-panel-border bg-panel-background px-6 py-16 text-center">
              <Search className="h-8 w-8 text-text-muted" />
              <p className="text-sm font-medium text-text-primary">没有找到符合条件的模型</p>
              <p className="text-xs text-text-secondary">尝试调整搜索内容或筛选条件</p>
              <Button
                variant="outline"
                size="sm"
                className="mt-1 shadow-none"
                onClick={clearFilters}
              >
                清除筛选
              </Button>
            </div>
          ) : (
            filtered.map((m) => (
              <LibraryCard
                key={m.id}
                model={m}
                selected={selectedIds.has(m.id)}
                onToggleSelect={toggleSelect}
                downloadingId={lib.downloadingId}
                progress={lib.progress}
                onDownload={lib.download}
                onCancelDownload={lib.cancelDownload}
                onUse={lib.setCurrent}
                onImport={openImport}
                onOpenDir={(model) => {
                  void api.openModelDirectory({ id: model.id });
                }}
                onDetail={(model) => {
                  setDetailModel(model);
                  setDetailOpen(true);
                }}
                onDelete={openConfirm}
                onRemove={openConfirm}
              />
            ))
          )}
        </div>
      </div>

      <LibraryBulkBar
        filtered={filtered}
        selectedIds={selectedIds}
        onSelectAll={selectAllFiltered}
        onClear={() => setSelectedIds(new Set())}
        onBatchDelete={batchDelete}
      />

      {/* 对话框 */}
      <ModelConfirmDialog
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        model={confirmModel}
        onConfirm={confirmDelete}
      />
      <ModelDetailDialog
        open={detailOpen}
        onClose={() => setDetailOpen(false)}
        model={detailModel}
      />
      <AddLocalModelDialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        registryId={addRegistryId}
        onAddLocal={lib.addLocal}
      />
    </div>
  );
}
