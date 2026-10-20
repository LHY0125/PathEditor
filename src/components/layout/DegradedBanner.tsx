import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';

/** 历史库的可用性状态。`null` = 尚未探测完成。 */
type HistoryAvailability = 'ok' | 'degraded';

/**
 * 降级条幅（spec §7.10：**常驻**错误条幅 + 状态栏明示）。
 *
 * **为什么挂在 AppShell 而不是历史面板里**（J-9 整改）：DB 打不开时，用户
 * 只要不切到「历史记录」标签页就完全看不到提示，会以为历史功能正常，
 * 直到某天点进去才发现全是空——而此时可能已经积累了大量「应该有记录却没有」
 * 的操作。降级必须**在启动时就可见**，与当前标签页无关。
 *
 * 探测方式：启动时调用 `history_get_current`。
 * - 返回 `null` 或快照 → 库可用（`null` 只是「还没有当前快照」，不是故障）；
 * - 抛错且 `code` 为 `database` / `crypto` → 降级。
 *
 * **不做定时轮询**：降级是启动期状态（打开失败就一直是 None），
 * 轮询只会增加无谓的 IPC。恢复需要重启，届时这里会重新探测。
 */
export function DegradedBanner() {
  const { t } = useTranslation();
  const [state, setState] = useState<HistoryAvailability | null>(null);
  const [detail, setDetail] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        await backend.historyGetCurrent();
        if (!cancelled) setState('ok');
      } catch (e) {
        if (cancelled) return;
        // parseCoreError 抛出的是普通对象 {code, message}，不是 Error 实例
        const code =
          typeof e === 'object' && e !== null && 'code' in e
            ? String((e as { code: unknown }).code)
            : '';
        const message =
          typeof e === 'object' && e !== null && 'message' in e
            ? String((e as { message: unknown }).message)
            : String(e);
        if (code === 'database' || code === 'crypto') {
          setState('degraded');
          setDetail(message);
        } else {
          setState('ok');
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (state !== 'degraded') return null;

  return (
    <div
      className="flex items-center gap-2 px-4 py-1.5 text-xs"
      style={{
        background: 'var(--app-warning)',
        color: 'var(--app-accent-fg)',
      }}
      role="alert"
    >
      <span className="font-semibold">{t('history.degradedTitle', '历史功能不可用')}</span>
      <span className="truncate opacity-90" title={detail}>
        {detail || t('history.degradedDetail', '历史库无法打开，当前处于降级模式')}
      </span>
      <span className="ml-auto shrink-0 opacity-90">
        {t('history.degradedHint', '环境变量编辑不受影响；重启后可重试')}
      </span>
    </div>
  );
}
