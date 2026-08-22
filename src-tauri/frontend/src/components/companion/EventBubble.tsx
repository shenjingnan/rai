import { useEffect, useRef, useState } from "react";
import { createToastManager, Toaster } from "@/components/ui/toast-deck";
import { onDshSpeak } from "@/lib/tauri";

/** 卡片自动消失时间（deck 展开期间冻结，收回后继续走剩余时间）。 */
const VISIBLE_MS = 8000;
/** deck 容量上限（超出标记 data-limited 隐藏让位，前面的过期后自动浮现）。 */
const LIMIT = 3;
/** 自持时钟轮询周期：关闭过期卡片 / 展开期间冻结 deadline。 */
const CLOCK_INTERVAL_MS = 500;

/**
 * 桌宠事件 toast deck：订阅 `dsh-speak`，在窗口顶部居中堆叠展示模板台词。
 *
 * - 多条消息折成一副牌（最新在前，旧卡探边微缩），hover 展开扇出、移开收回；
 * - 容量 3、每条 8s 自动消失（展开时冻结）；全文有对话记录兜底；
 * - 卡片拦截 mousedown/contextmenu 冒泡（不触发窗口拖动/右键菜单）；
 * - 点击穿透/置底模式下系统级屏蔽鼠标事件，deck 退化为纯展示（不可展开）。
 *
 * 过期由自持时钟驱动而非 Base UI 的计时器：后者在窗口失焦时暂停计时
 * （`expandedOrOutOfFocus` 选择器），而桌宠是 nonactivating panel 永不获焦，
 * 卡片会永久滞留（真机验证过）。故入队即 `timeout: 0` 关库计时，到期由
 * interval 主动 `manager.close`；展开态（`[data-expanded]`）顺延 deadline，
 * 保留 hover 暂停细读的体验。
 */
export function EventBubble() {
  // 每实例私有 manager：桌宠窗口仅一个消费方，也天然隔离测试间状态。
  const [manager] = useState(() => createToastManager());
  // toast id → 到期时间戳。
  const deadlines = useRef(new Map<string, number>());

  useEffect(() => {
    const unlisten = onDshSpeak(({ text }) => {
      const id = manager.add({ title: text, timeout: 0 });
      deadlines.current.set(id, Date.now() + VISIBLE_MS);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [manager]);

  useEffect(() => {
    const timer = setInterval(() => {
      const now = Date.now();
      // 展开态（hover / 键盘焦点）冻结时钟：所有 deadline 顺延一个周期。
      const frozen = document.querySelector('[data-slot="toast"][data-expanded]') !== null;
      for (const [id, at] of deadlines.current) {
        if (frozen) {
          deadlines.current.set(id, at + CLOCK_INTERVAL_MS);
        } else if (now >= at) {
          manager.close(id);
          deadlines.current.delete(id);
        }
      }
    }, CLOCK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [manager]);

  return <Toaster toastManager={manager} limit={LIMIT} />;
}
