/**
 * 应用截图展示区：假浏览器窗口框内嵌 home.png。
 * 对应 Tailwind UI「bordered app screenshot」变体。
 */
export function ScreenshotSection() {
  return (
    <div className="mx-auto mt-16 max-w-5xl px-6 sm:mt-24 lg:px-8">
      <div className="relative rounded-2xl bg-fd-card shadow-2xl ring-1 ring-fd-border">
        <div className="flex items-center gap-2 border-b border-fd-border px-4 py-3">
          <span className="size-3 rounded-full bg-[#ff5f57]" />
          <span className="size-3 rounded-full bg-[#febc2e]" />
          <span className="size-3 rounded-full bg-[#28c840]" />
          <div className="ms-3 flex-1 truncate rounded-md bg-fd-muted px-3 py-1 text-xs text-fd-muted-foreground">
            zapmomo · 主面板「概览」
          </div>
        </div>
        <img
          src="/screenshots/home.png"
          alt="ZapMomo 桌面应用概览页"
          width={1600}
          height={1030}
          loading="lazy"
          className="h-auto w-full rounded-b-2xl"
        />
      </div>
      <p className="mt-4 text-center text-sm text-fd-muted-foreground">
        桌面应用「概览」页：展示当前伙伴与 AI 能力状态
      </p>
    </div>
  );
}
