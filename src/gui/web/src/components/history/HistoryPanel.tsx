import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';
import { useHistoryStore } from '@/store/history-store';
import { CompareDialog } from './CompareDialog';
import { SnapshotDetail } from './SnapshotDetail';

function formatTime(ms: number): string {
  return new Date(ms).toLocaleString();
}

/**
 * 历史时间线面板。
 *
 * 列出事件、选中查看快照、显示掩码后的变量值，并支持恢复快照与筛选。
 * 所有文案走 i18n：`action.*` / `source.*` 等键在
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
  const restoring = useHistoryStore((s) => s.restoring);
  const load = useHistoryStore((s) => s.load);
  const selectEvent = useHistoryStore((s) => s.selectEvent);
  const restore = useHistoryStore((s) => s.restore);
  const filter = useHistoryStore((s) => s.filter);
  const setFilter = useHistoryStore((s) => s.setFilter);
  const undoLast = useHistoryStore((s) => s.undoLast);

  // 多选两个事件做对比（存的是 snapshotUid，不是事件 uid——
  // 对比的对象是快照，而事件只是「哪两个快照可比」的入口）
  const [compareSel, setCompareSel] = useState<string[]>([]);
  const [comparePair, setComparePair] = useState<[string, string] | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  /**
   * 恢复快照（破坏性：覆盖当前 PATH）。
   *
   * 二次确认走异步对话框；**IPC 失败按「取消」处理**（fail-closed），
   * 绝不静默恢复——与仓库里其它破坏性操作同一口径。
   */
  /**
   * 跨会话撤销：恢复最近一次可撤销操作前的快照。
   *
   * 与 `handleRestore` 同样走二次确认——它也是破坏性操作（覆盖当前 PATH）。
   */
  const handleUndo = async () => {
    const confirmed = await backend.confirmDialog(t('history.undoConfirm')).catch(() => false);
    if (!confirmed) return;
    const undone = await undoLast().catch(() => null);
    if (!undone) {
      // 没有可撤销的记录：给出明确反馈而不是静默无反应
      useHistoryStore.setState({ error: t('history.undoNothing') });
    }
  };

  const handleRestore = async (uid: string) => {
    const confirmed = await backend.confirmDialog(t('history.restoreConfirm')).catch(() => false);
    if (!confirmed) return;
    await restore(uid).catch(() => {
      // 错误已由 store 写入 `error`，条幅会显示
    });
  };

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
        <div className="flex items-center gap-1.5">
          {/* 筛选下拉：core 与 IPC 支持 action/source/hive/target/时间范围，
              界面目前只暴露最常用的 action/source/hive 三个维度。 */}
          <select
            className="field-input"
            value={filter.action ?? ''}
            onChange={(e) => void setFilter({ ...filter, action: e.target.value || undefined })}
            aria-label={t('history.filterAction')}
          >
            <option value="">{t('history.filterAllActions')}</option>
            <option value="save">{t('history.action.save')}</option>
            <option value="apply">{t('history.action.apply')}</option>
            <option value="restore">{t('history.action.restore')}</option>
            <option value="import">{t('history.action.import')}</option>
            <option value="envSet">{t('history.action.envSet')}</option>
            <option value="envAdd">{t('history.action.envAdd')}</option>
            <option value="envRemove">{t('history.action.envRemove')}</option>
            <option value="undo">{t('history.action.undo')}</option>
            <option value="redo">{t('history.action.redo')}</option>
          </select>
          <select
            className="field-input"
            value={filter.source ?? ''}
            onChange={(e) => void setFilter({ ...filter, source: e.target.value || undefined })}
            aria-label={t('history.filterSource')}
          >
            <option value="">{t('history.filterAllSources')}</option>
            <option value="gui">{t('history.source.gui')}</option>
            <option value="cli">{t('history.source.cli')}</option>
            <option value="import">{t('history.source.import')}</option>
            <option value="restore">{t('history.source.restore')}</option>
            <option value="migration">{t('history.source.migration')}</option>
            <option value="sync">{t('history.source.sync')}</option>
          </select>
          <select
            className="field-input"
            value={filter.hive ?? ''}
            onChange={(e) => void setFilter({ ...filter, hive: e.target.value || undefined })}
            aria-label={t('history.filterHive')}
          >
            <option value="">{t('history.filterAllHives')}</option>
            <option value="system">{t('history.hiveSystem')}</option>
            <option value="user">{t('history.hiveUser')}</option>
          </select>
          <button
            type="button"
            className="toolbar-btn"
            onClick={() => void handleUndo()}
            disabled={restoring || events.length === 0}
            title={t('history.undoHint')}
          >
            {t('history.undoLast')}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            onClick={() => void load()}
            disabled={loading}
          >
            {t('history.refresh')}
          </button>
        </div>
      </div>

      {/* 降级模式条幅：DB 不可用时历史不可用，但注册表编辑照常 */}
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

      {/* 对比：勾选两个事件后启用。选中顺序决定 left/right（先选的作 left）。 */}
      {compareSel.length > 0 && (
        <div className="mb-2 flex items-center gap-2 text-xs">
          <button
            type="button"
            className="tab-pill"
            disabled={compareSel.length !== 2}
            onClick={() => {
              if (compareSel.length === 2) {
                setComparePair([compareSel[0], compareSel[1]]);
              }
            }}
          >
            {t('compare.open', { count: compareSel.length })}
          </button>
          <button type="button" className="tab-pill" onClick={() => setCompareSel([])}>
            {t('compare.clearSelection')}
          </button>
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
              <li key={e.uid} className="flex items-start">
                {/* 对比多选：复选框必须在按钮**外面**——不能嵌套交互元素。
                    只有带 snapshotUid 的事件才能参与对比（对比的对象是快照）。 */}
                {clickable && (
                  <input
                    type="checkbox"
                    className="mt-2 ml-2 shrink-0"
                    aria-label={t('compare.selectForCompare')}
                    checked={compareSel.includes(e.snapshotUid as string)}
                    onChange={() => {
                      const uid = e.snapshotUid as string;
                      setCompareSel((prev) =>
                        prev.includes(uid)
                          ? prev.filter((x) => x !== uid)
                          : // 最多选两个：再选第三个时挤掉最早的那个
                            [...prev, uid].slice(-2),
                      );
                    }}
                  />
                )}
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
          {!detailLoading && detail && (
            <>
              <SnapshotDetail snapshot={detail} />
              <div className="border-t px-3 py-2" style={{ borderColor: 'var(--app-border)' }}>
                <button
                  type="button"
                  className="toolbar-btn w-full"
                  disabled={restoring}
                  onClick={() => void handleRestore(detail.uid)}
                >
                  {restoring ? t('history.restoring') : t('history.restore')}
                </button>
                <p className="mt-1 text-xs" style={{ color: 'var(--app-fg-faint)' }}>
                  {t('history.restoreHint')}
                </p>
              </div>
            </>
          )}
          {!detailLoading && !detail && (
            <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.pickOne')}
            </p>
          )}
        </div>
      </div>

      {comparePair !== null && (
        <CompareDialog
          leftUid={comparePair[0]}
          rightUid={comparePair[1]}
          onClose={() => setComparePair(null)}
        />
      )}
    </div>
  );
}
