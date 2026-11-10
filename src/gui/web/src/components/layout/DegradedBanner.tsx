import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';
import { backend } from '@/services/backend';

/** 历史库的可用性状态。`null` = 尚未探测完成。 */
type HistoryAvailability = 'ok' | 'degraded';

/**
 * 降级条幅。
 *
 * **为什么挂在 AppShell 而不是历史面板里**：DB 打不开时，用户
 * 只要不切到「历史记录」标签页就完全看不到提示，会以为历史功能正常，
 * 直到某天点进去才发现全是空——而此时可能已经积累了大量「应该有记录却没有」
 * 的操作。降级必须**在启动时就可见**，与当前标签页无关。
 *
 * 探测方式：启动时调用 `history_get_current`。
 * - 返回 `null` 或快照 → 库可用（`null` 只是「还没有当前快照」，不是故障）；
 * - 抛错且 `code` 为 `database` / `crypto` / `migration` → 降级。
 *
 * **不做定时轮询**：降级是启动期状态（打开失败就一直是 None），
 * 轮询只会增加无谓的 IPC。恢复需要重启，届时这里会重新探测。
 */
export function DegradedBanner() {
  const { t } = useTranslation();
  const [state, setState] = useState<HistoryAvailability | null>(null);
  const [detail, setDetail] = useState<string>('');
  const [code, setCode] = useState<string>('');

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
        if (code === 'database' || code === 'crypto' || code === 'migration') {
          setState('degraded');
          setDetail(message);
          setCode(code);
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

  // 只有 schema 不兼容与旧 SQLCipher 库是“确定需要重建”的场景；
  // 其它 database 错误（锁、I/O、损坏）不能直接建议删除。
  const needsRebuild = code === 'migration' || detail.includes('旧版加密数据库');

  return (
    <div
      className="flex items-center gap-2 px-4 py-1.5 text-xs"
      style={{
        background: 'var(--app-warning)',
        color: 'var(--app-warning-fg)',
      }}
      role="alert"
    >
      <span className="font-semibold">{t('history.degradedTitle', '历史功能不可用')}</span>
      <span className="truncate opacity-90" title={detail}>
        {detail || t('history.degradedDetail', '历史库无法打开，当前处于降级模式')}
      </span>
      <span className="ml-auto flex shrink-0 items-center gap-2 opacity-90">
        <span>
          {needsRebuild
            ? t('history.degradedHintMigration', '请打开数据目录，备份后删除旧库再重启')
            : code === 'database'
              ? t('history.degradedHintDatabase', '请打开数据目录检查并先备份，不要直接删除数据库')
              : t('history.degradedHint', '环境变量编辑不受影响；请打开数据目录检查旧库')}
        </span>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void backend.openDataDir().catch(() => {})}
        >
          {t('history.degradedOpenDataDir', '打开数据目录')}
        </Button>
      </span>
    </div>
  );
}
