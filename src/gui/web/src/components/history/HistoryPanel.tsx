import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';
import type { RestoreOutcome } from '@/core/env-backup';
import { errorMessage } from '@/core/error';
import { Button } from '@/components/ui/Button';
import { useHistoryStore } from '@/store/history-store';
import { CompareDialog } from './CompareDialog';
import { MultiCompareDialog } from './MultiCompareDialog';
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
  const loadMore = useHistoryStore((s) => s.loadMore);
  const selectEvent = useHistoryStore((s) => s.selectEvent);
  const restore = useHistoryStore((s) => s.restore);
  const restoreFull = useHistoryStore((s) => s.restoreFull);
  const filter = useHistoryStore((s) => s.filter);
  const setFilter = useHistoryStore((s) => s.setFilter);
  const undoLast = useHistoryStore((s) => s.undoLast);
  const openSnapshot = useHistoryStore((s) => s.openSnapshot);
  // 固定快照：详情里切换固定后（detail.pinned 变化）自动刷新这一行
  const [pinned, setPinned] = useState<{ uid: string; createdAt: number; note: string | null }[]>(
    [],
  );
  useEffect(() => {
    let alive = true;
    backend
      .historyListPinned()
      .then((list) => {
        if (alive) setPinned(list);
      })
      .catch(() => {
        // DB 降级时固定列表为空即可，不覆盖主错误条幅
        if (alive) setPinned([]);
      });
    return () => {
      alive = false;
    };
  }, [detail?.uid, detail?.pinned]);

  // 多选两个事件做对比（存的是 snapshotUid，不是事件 uid——
  // 对比的对象是快照，而事件只是「哪两个快照可比」的入口）
  const [compareSel, setCompareSel] = useState<string[]>([]);
  const [comparePair, setComparePair] = useState<[string, string] | null>(null);
  const [compareMany, setCompareMany] = useState<string[] | null>(null);
  /** 选到第三条时提示「最早的一条已被挤掉」，避免静默改变选择。 */
  const [compareNotice, setCompareNotice] = useState(false);
  // 结果按快照 uid 归属：切换快照后旧结果自然不再显示，无需在 effect 里 setState。
  const [envOutcome, setEnvOutcome] = useState<{ uid: string; result: RestoreOutcome } | null>(
    null,
  );
  const [envError, setEnvError] = useState<{ uid: string; message: string } | null>(null);

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
    try {
      const undone = await undoLast();
      if (!undone) {
        // 没有可撤销的记录：给出明确反馈而不是静默无反应
        useHistoryStore.setState({ error: t('history.undoNothing') });
      }
    } catch {
      // `restore` 已把真实错误（permissionDenied / database …）写进 store，
      // 这里不能再覆盖成「没有可撤销记录」，否则用户会按错误提示排查。
    }
  };

  /** 导出当前筛选下的事件为 CSV（走 Blob 下载，与 PATH 导出一致）。 */
  const handleExport = async () => {
    try {
      const content = await backend.historyExportEvents(filter, 'csv');
      const blob = new Blob([content], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'patheditor_history.csv';
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      useHistoryStore.setState({ error: errorMessage(error) });
    }
  };

  const handleRestore = async (uid: string) => {
    const confirmed = await backend.confirmDialog(t('history.restoreConfirm')).catch(() => false);
    if (!confirmed) return;
    await restore(uid).catch(() => {
      // 错误已由 store 写入 `error`，条幅会显示
    });
  };

  /** 一次性恢复整份快照（PATH + env）。 */
  const handleRestoreFull = async (uid: string) => {
    const confirmed = await backend
      .confirmDialog(t('history.restoreFullConfirm'))
      .catch(() => false);
    if (!confirmed) return;
    await restoreFull(uid).catch(() => {
      // 错误已由 store 写入 `error`
    });
  };

  /** 恢复快照里的环境变量；与 PATH 恢复分开，避免一次操作同时改两类状态。 */
  const handleRestoreEnv = async (uid: string, count: number) => {
    const confirmed = await backend
      .confirmDialog(t('history.restoreEnvConfirm', { count }))
      .catch(() => false);
    if (!confirmed) return;
    try {
      const outcome = await backend.historyRestoreEnvSnapshot(uid, true);
      setEnvOutcome({ uid, result: outcome });
      setEnvError(null);
    } catch (e) {
      setEnvOutcome(null);
      setEnvError({ uid, message: errorMessage(e) });
    }
  };

  return (
    <div className="history-panel flex min-h-0 flex-1 flex-col gap-3">
      {pinned.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('history.pinnedSnapshots')}
          </span>
          {pinned.map((item) => (
            <Button
              key={item.uid}
              variant="ghost"
              onClick={() => void openSnapshot(item.uid)}
              title={formatTime(item.createdAt)}
            >
              {item.note ?? formatTime(item.createdAt)}
            </Button>
          ))}
        </div>
      )}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-baseline gap-2">
          <h2 className="text-sm font-semibold" style={{ color: 'var(--app-fg)' }}>
            {t('history.title')}
          </h2>
          <span className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('history.showing', { shown: events.length, total })}
          </span>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-1.5">
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
            <option value="revealEnv">{t('history.action.revealEnv')}</option>
            <option value="drift">{t('history.action.drift')}</option>
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
          <Button
            variant="ghost"
            onClick={() => void handleUndo()}
            disabled={restoring || events.length === 0}
            title={t('history.undoHint')}
          >
            {t('history.undoLast')}
          </Button>
          <Button
            variant="ghost"
            onClick={() => void handleExport()}
            disabled={events.length === 0}
            title={t('history.exportHint')}
          >
            {t('history.exportCsv')}
          </Button>
          <Button variant="ghost" onClick={() => void load()} disabled={loading}>
            {t('history.refresh')}
          </Button>
        </div>
      </div>

      {/* 降级模式条幅：DB 不可用时历史不可用，但注册表编辑照常 */}
      {error && (
        <div
          className="rounded-lg px-3 py-2 text-xs"
          style={{
            background: 'var(--app-warning)',
            color: 'var(--app-warning-fg)',
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
          <button
            type="button"
            className="tab-pill"
            onClick={() => {
              setCompareNotice(false);
              setCompareSel([]);
            }}
          >
            {t('compare.clearSelection')}
          </button>
          {compareNotice && (
            <span className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('compare.maxTwo')}
            </span>
          )}
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
                      setCompareSel((prev) => {
                        if (prev.includes(uid)) {
                          setCompareNotice(false);
                          return prev.filter((x) => x !== uid);
                        }
                        return [...prev, uid];
                      });
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
                        <span className="ml-1" style={{ color: 'var(--app-warning-text)' }}>
                          ({e.result === 'partial' ? t('history.partial') : t('history.failure')})
                        </span>
                      )}
                    </span>
                    <span style={{ color: 'var(--app-fg-muted)' }}>{formatTime(e.createdAt)}</span>
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
          {events.length < total && (
            <li className="p-2">
              <Button
                variant="ghost"
                className="w-full"
                disabled={loading}
                onClick={() => void loadMore()}
              >
                {loading ? t('history.loading') : t('history.loadMore')}
              </Button>
            </li>
          )}
        </ul>

        {/* 详情 */}
        <div
          className={`min-h-0 w-80 shrink-0 overflow-y-auto rounded-lg border ${
            !detailLoading && !detail ? 'history-detail-empty' : ''
          }`}
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
                <Button
                  variant="primary"
                  className="w-full"
                  disabled={restoring}
                  onClick={() => void handleRestoreFull(detail.uid)}
                >
                  {restoring ? t('history.restoring') : t('history.restoreFull')}
                </Button>
                <Button
                  variant="ghost"
                  className="w-full"
                  disabled={restoring}
                  onClick={() => void handleRestore(detail.uid)}
                >
                  {t('history.restore')}
                </Button>
                {detail.env.length > 0 && (
                  <Button
                    variant="ghost"
                    className="w-full"
                    disabled={restoring}
                    onClick={() => void handleRestoreEnv(detail.uid, detail.env.length)}
                  >
                    {t('history.restoreEnv')}
                  </Button>
                )}
                <p className="mt-1 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
                  {t('history.restoreHint')}
                </p>
                {envOutcome !== null && envOutcome.uid === detail.uid && (
                  <>
                    <p
                      className="mt-1 text-xs"
                      style={{
                        color:
                          envOutcome.result.failures.length > 0
                            ? 'var(--app-warning-text)'
                            : 'var(--app-success)',
                      }}
                    >
                      {envOutcome.result.failures.length > 0
                        ? t('history.restoreEnvFailed', {
                            applied: envOutcome.result.applied,
                            failed: envOutcome.result.failures.length,
                          })
                        : t('history.restoreEnvDone', {
                            applied: envOutcome.result.applied,
                          })}
                    </p>
                    {envOutcome.result.failures.length > 0 && (
                      <ul className="mt-1 text-xs" style={{ color: 'var(--app-danger)' }}>
                        {envOutcome.result.failures.map((failure) => (
                          <li key={failure}>{failure}</li>
                        ))}
                      </ul>
                    )}
                  </>
                )}
                {envError !== null && envError.uid === detail.uid && (
                  <p className="mt-1 text-xs" style={{ color: 'var(--app-danger)' }}>
                    {envError.message}
                  </p>
                )}
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

      {compareMany !== null && (
        <MultiCompareDialog uids={compareMany} onClose={() => setCompareMany(null)} />
      )}

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
