import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '@/store/history-store';
import { SnapshotDetail } from './SnapshotDetail';

const ACTION_LABEL: Record<string, string> = {
  save: '保存 PATH',
  apply: '应用配置',
  restore: '恢复快照',
  import: '导入',
  envSet: '修改变量',
  envAdd: '新增变量',
  envRemove: '删除变量',
  undo: '撤销',
  redo: '重做',
};

const SOURCE_LABEL: Record<string, string> = {
  gui: '图形界面',
  cli: '命令行',
  import: '文件导入',
  restore: '恢复',
  migration: '迁移',
  sync: '同步',
};

function formatTime(ms: number): string {
  return new Date(ms).toLocaleString();
}

/**
 * 历史时间线面板（M1 最小可用）。
 *
 * 只做三件事：列出事件、选中查看快照、显示掩码后的变量值。
 * 恢复操作与筛选留到 M2（见回执「未覆盖项」）。
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
            {t('history.title', '历史记录')}
          </h2>
          <span className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('history.count', '共 {{total}} 条', { total })}
          </span>
        </div>
        <button
          type="button"
          className="toolbar-btn"
          onClick={() => void load()}
          disabled={loading}
        >
          {t('history.refresh', '刷新')}
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
          {t('history.unavailable', '历史不可用（降级模式）：{{message}}', { message: error })}
          <span className="ml-2 opacity-80">
            {t('history.editingStillWorks', '环境变量编辑不受影响。')}
          </span>
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
              {t('history.loading', '加载中…')}
            </li>
          )}
          {!loading && events.length === 0 && !error && (
            <li className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.empty', '还没有历史记录')}
            </li>
          )}
          {events.map((e) => (
            <li key={e.uid}>
              <button
                type="button"
                className="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left text-xs transition-colors"
                style={{ color: 'var(--app-fg)' }}
                onMouseEnter={(ev) => {
                  ev.currentTarget.style.background = 'var(--app-hover)';
                }}
                onMouseLeave={(ev) => {
                  ev.currentTarget.style.background = '';
                }}
                onClick={() => void selectEvent(e)}
              >
                <span className="flex w-full items-center justify-between gap-2">
                  <span className="font-medium">
                    {ACTION_LABEL[e.action] ?? e.action}
                    {e.result !== 'success' && (
                      <span className="ml-1" style={{ color: 'var(--app-warning)' }}>
                        ({e.result === 'partial' ? '部分成功' : '失败'})
                      </span>
                    )}
                  </span>
                  <span style={{ color: 'var(--app-fg-faint)' }}>{formatTime(e.createdAt)}</span>
                </span>
                <span style={{ color: 'var(--app-fg-muted)' }}>
                  {SOURCE_LABEL[e.source] ?? e.source}
                  {e.target ? ` · ${e.target}` : ''}
                  {e.hive ? ` · ${e.hive === 'system' ? '系统' : '用户'}` : ''}
                </span>
              </button>
            </li>
          ))}
        </ul>

        {/* 详情 */}
        <div
          className="min-h-0 w-80 shrink-0 overflow-y-auto rounded-lg border"
          style={{ borderColor: 'var(--app-border)' }}
        >
          {detailLoading && (
            <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.detailLoading', '加载快照…')}
            </p>
          )}
          {!detailLoading && detail && <SnapshotDetail snapshot={detail} />}
          {!detailLoading && !detail && (
            <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('history.pickOne', '选择一条记录查看快照')}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
