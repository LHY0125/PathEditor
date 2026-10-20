import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '@/store/history-store';
import { SnapshotDetail } from './SnapshotDetail';

function formatTime(ms: number): string {
  return new Date(ms).toLocaleString();
}

/**
 * 历史时间线面板（M1 最小可用）。
 *
 * 只做三件事：列出事件、选中查看快照、显示掩码后的变量值。
 * 恢复操作与筛选留到 M2（见回执「未覆盖项」）。
 *
 * **所有文案走 i18n**（J-2 整改）：`action.*` / `source.*` 等键在
 * `src/i18n/locales/{zh-CN,en}.json` 的 `history` 段里。
 * 用 `defaultValue` 只是兜底，不代表可以省掉 locale 条目。
 */
export function HistoryPanel() {
  const { t } = useTranslation();
  const events = useHistoryStore((s) => s.events);
  const total = useHistoryStore((s) => s.total);
  const loading = useHistoryStore((s) => s.loading);
  const error = useHistoryStore((s) => s.error);
  const detail = useHistoryStore((s) => s.detail);
  const detailLoading = useHistoryStore((s) => s.detailLoading);
  const load = useHistoryStore((s) => s.load);
  const selectEvent = useHistoryStore((s) => s.selectEvent);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between">
        <div className="flex items-baseline gap-2">
          <h2 className="text-sm font-semibold" style={{ color: 'var(--app-fg)' }}>
            {t('history.title')}
          </h2>
          <span className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('history.count', { total })}
          </span>
        </div>
        <button
          type="button"
          className="toolbar-btn"
          onClick={() => void load()}
          disabled={loading}
        >
          {t('history.refresh')}
        </button>
      </div>

      {/* 降级模式条幅：DB 不可用时历史不可用，但注册表编辑照常（spec D16） */}
      {error && (
        <div
          className="rounded-lg px-3 py-2 text-xs"
          style={{
            background: 'var(--app-warning)',
            color: 'var(--app-accent-fg)',
          }}
          role="alert"
        >
          {t('history.unavailable', { message: error })}
          <span className="ml-2 opacity-80">{t('history.editingStillWorks')}</span>
        </div>
      )}

      <div className="flex min-h-0 flex-1 gap-3">
        {/* 时间线 */}
        <ul
          className="min-h-0 flex-1 overflow-y-auto rounded-lg border"
          style={{ borderColor: 'var(--app-border)' }}
        >
          {loading && events.length === 0 && (
            <li className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.loading')}
            </li>
          )}
          {!loading && events.length === 0 && !error && (
            <li className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.empty')}
            </li>
          )}
          {events.map((e) => {
            // 没有关联快照的事件不可点开详情：明确提示而不是让右栏空白
            const clickable = e.snapshotUid !== null;
            return (
              <li key={e.uid}>
                <button
                  type="button"
                  className="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left text-xs transition-colors"
                  style={{ color: 'var(--app-fg)', cursor: clickable ? 'pointer' : 'default' }}
                  onMouseEnter={(ev) => {
                    if (clickable) ev.currentTarget.style.background = 'var(--app-hover)';
                  }}
                  onMouseLeave={(ev) => {
                    ev.currentTarget.style.background = '';
                  }}
                  onClick={() => void selectEvent(e)}
                  disabled={!clickable}
                >
                  <span className="flex w-full items-center justify-between gap-2">
                    <span className="font-medium">
                      {t(`history.action.${e.action}`, { defaultValue: e.action })}
                      {e.result !== 'success' && (
                        <span className="ml-1" style={{ color: 'var(--app-warning)' }}>
                          ({e.result === 'partial' ? t('history.partial') : t('history.failure')})
                        </span>
                      )}
                    </span>
                    <span style={{ color: 'var(--app-fg-faint)' }}>{formatTime(e.createdAt)}</span>
                  </span>
                  <span style={{ color: 'var(--app-fg-muted)' }}>
                    {t(`history.source.${e.source}`, { defaultValue: e.source })}
                    {e.target ? ` · ${e.target}` : ''}
                    {e.hive
                      ? ` · ${e.hive === 'system' ? t('history.hiveSystem') : t('history.hiveUser')}`
                      : ''}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>

        {/* 详情 */}
        <div
          className="min-h-0 w-80 shrink-0 overflow-y-auto rounded-lg border"
          style={{ borderColor: 'var(--app-border)' }}
        >
          {detailLoading && (
            <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.detailLoading')}
            </p>
          )}
          {!detailLoading && detail && <SnapshotDetail snapshot={detail} />}
          {!detailLoading && !detail && (
            <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.pickOne')}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
