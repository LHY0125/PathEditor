import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';
import { maskConflictValue, useSyncStore, type SyncNotice } from '@/store/sync-store';
import type { SyncConflict } from '@/services/backend';
import { Button } from '@/components/ui/Button';

/**
 * 把 store 的**结构化** notice 渲染成文案。
 *
 * store 不产出中文/英文字符串——文案必须过 `t()`，否则切到英文界面时
 * 这条主反馈仍是中文。
 *
 * `imported` 分两种文案：有 `skippedUnsupported` 时必须**显式说出**
 * 「这些没同步」，否则「应用 N 条」会被理解成全部成功。
 */
function renderNotice(
  notice: SyncNotice,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  switch (notice.kind) {
    case 'exported':
      return t('sync.noticeExported', {
        count: notice.changeCount,
        path: notice.path,
      });
    case 'imported':
      // 顺序有意义：先说「有 N 条根本没同步」（最需要用户知道），
      // 再说「有 N 条被改名了」（用户会发现名字变了，必须解释），
      // 最后才是普通的成功提示。
      if (notice.report.skippedUnsupported > 0) {
        return t('sync.noticeImportedUnsupported', {
          applied: notice.report.applied,
          skipped: notice.report.skipped,
          conflicts: notice.report.conflicts,
          unsupported: notice.report.skippedUnsupported,
        });
      }
      if (notice.report.renamedProfiles > 0) {
        return t('sync.noticeImportedRenamed', {
          applied: notice.report.applied,
          skipped: notice.report.skipped,
          conflicts: notice.report.conflicts,
          renamed: notice.report.renamedProfiles,
        });
      }
      return t('sync.noticeImported', {
        applied: notice.report.applied,
        skipped: notice.report.skipped,
        conflicts: notice.report.conflicts,
      });
    case 'resolved':
      return t('sync.resolvedAs', { resolution: notice.resolution });
  }
}

/**
 * 同步中心。
 *
 * ## 口令只活在组件里
 *
 * `passphrase` 是局部 `useState`，**不写入任何 store**。导出/导入结束后立刻
 * 清空。输入框用 `type="password"`，不提供「记住」选项—— 要求
 * 「口令不落盘、不记忆」。
 *
 * ## 同步不写注册表
 *
 * 导入只改数据库。界面必须把这句话写出来，否则用户会以为导入完 PATH 就变了。
 *
 * ## 冲突按实体类型分组
 *
 * 一条冲突对应「同一实体同一字段被两台设备同时改」。分组让用户能按
 * 「配置 / 快照」逐个处理，而不是面对一长串无结构的列表。
 *
 * ## 视觉规范
 *
 * 布局与配色全部走 `globals.css` 的 `.sync-*` 类与语义 token；
 * 动作按钮用 `Button` 原语，不再拿 `.tab-pill` 当按钮用。
 */
export function SyncCenter() {
  const { t } = useTranslation();
  const {
    status,
    conflicts,
    loading,
    error,
    notice,
    refresh,
    exportBundle,
    importBundle,
    resolve,
  } = useSyncStore();

  // 口令与文件路径都是**局部的**，不进 store
  const [passphrase, setPassphrase] = useState('');
  const [filePath, setFilePath] = useState('');
  const [includeResolved, setIncludeResolved] = useState(false);
  /** 正在执行的导出/导入：用于按钮文案，避免长耗时加密看起来没反应。 */
  const [busyAction, setBusyAction] = useState<'export' | 'import' | null>(null);

  useEffect(() => {
    void refresh();
    // 只在首次挂载时拉一次；后续操作各自刷新
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 按 entityType 分组，保持冲突 id 升序
  const grouped = useMemo(() => {
    const map = new Map<string, SyncConflict[]>();
    for (const c of conflicts) {
      const list = map.get(c.entityType) ?? [];
      list.push(c);
      map.set(c.entityType, list);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [conflicts]);

  const canSubmit = passphrase.length > 0 && filePath.length > 0 && !loading;

  const onExport = async () => {
    setBusyAction('export');
    try {
      const ok = await exportBundle(passphrase, filePath);
      // 不论成败都清空口令：失败后重试应由用户重新输入
      setPassphrase('');
      if (ok) setFilePath('');
    } finally {
      setBusyAction(null);
    }
  };

  const onImport = async () => {
    setBusyAction('import');
    try {
      const ok = await importBundle(passphrase, filePath);
      setPassphrase('');
      if (ok) setFilePath('');
    } finally {
      setBusyAction(null);
    }
  };

  /** 原生文件对话框：取消或非 Tauri 环境下静默保持原路径。 */
  const choosePath = async (mode: 'export' | 'import') => {
    const picked = await (
      mode === 'export'
        ? backend.pickSyncExportPath(t('sync.fileFilter'))
        : backend.pickSyncImportPath(t('sync.fileFilter'))
    ).catch(() => null);
    if (picked) setFilePath(picked);
  };

  return (
    <div className="sync-center">
      <header className="sync-header">
        <h2 className="text-sm font-semibold">{t('sync.title')}</h2>
        <span className="sync-muted text-xs">
          {status
            ? t('sync.status', {
                pending: status.pendingCount,
                conflicts: status.unresolvedConflicts,
              })
            : t('sync.statusUnknown')}
        </span>
        <Button size="sm" variant="ghost" onClick={() => void refresh()} disabled={loading}>
          {t('sync.refresh')}
        </Button>
      </header>

      {/* 口径说明：同步不写注册表，必须让用户看到 */}
      <p className="sync-note">{t('sync.registryNotice')}</p>

      {error !== null && (
        <div role="alert" className="sync-error">
          {error}
        </div>
      )}
      {notice !== null && (
        <div role="status" className="sync-notice">
          {renderNotice(notice, t)}
        </div>
      )}

      <section className="sync-section">
        <label className="sync-field">
          <span className="sync-label">{t('sync.passphrase')}</span>
          <input
            className="field-input"
            type="password"
            autoComplete="off"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            placeholder={t('sync.passphrasePlaceholder')}
          />
        </label>
        <label className="sync-field">
          <span className="sync-label">{t('sync.filePath')}</span>
          <span className="sync-file-row">
            <input
              className="field-input sync-file-input"
              type="text"
              value={filePath}
              onChange={(e) => setFilePath(e.target.value)}
              placeholder={t('sync.filePathPlaceholder')}
            />
            <Button size="sm" variant="ghost" onClick={() => void choosePath('export')}>
              {t('sync.chooseExport')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void choosePath('import')}>
              {t('sync.chooseImport')}
            </Button>
          </span>
        </label>
        <div className="sync-actions">
          <Button
            size="sm"
            variant="primary"
            disabled={!canSubmit || busyAction !== null}
            onClick={() => void onExport()}
          >
            {busyAction === 'export' ? t('sync.exporting') : t('sync.export')}
          </Button>
          <Button
            size="sm"
            disabled={!canSubmit || busyAction !== null}
            onClick={() => void onImport()}
          >
            {busyAction === 'import' ? t('sync.importing') : t('sync.import')}
          </Button>
        </div>
      </section>

      <section className="sync-section">
        <header className="sync-conflicts-header">
          <h3 className="text-xs font-semibold">{t('sync.conflicts')}</h3>
          <label className="sync-check">
            <input
              type="checkbox"
              checked={includeResolved}
              onChange={(e) => {
                setIncludeResolved(e.target.checked);
                void refresh(e.target.checked);
              }}
            />
            {t('sync.includeResolved')}
          </label>
        </header>

        {conflicts.length === 0 ? (
          <p className="sync-muted text-xs">{t('sync.noConflicts')}</p>
        ) : (
          grouped.map(([entityType, list]) => (
            <div key={entityType} className="sync-group">
              <h4 className="sync-muted text-xs font-semibold">
                {t(`sync.entityType.${entityType}`, entityType)}
              </h4>
              <ul className="sync-conflict-list">
                {list.map((c) => (
                  <li key={c.id} className="sync-conflict">
                    <div className="sync-conflict-head">
                      <code>{c.entityUid}</code>
                      {c.field !== null && <span>· {c.field}</span>}
                      <span className="sync-muted">
                        · {t('sync.fromDevice', { device: c.remoteDevice })}
                      </span>
                    </div>
                    <div className="sync-conflict-values">
                      {t('sync.localValue')}: {maskConflictValue(c.localValue)}
                    </div>
                    <div className="sync-conflict-values">
                      {t('sync.remoteValue')}: {maskConflictValue(c.remoteValue)}
                    </div>
                    {c.resolvedAt === null ? (
                      <div className="sync-conflict-actions">
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={loading}
                          onClick={() => void resolve(c.id, 'local')}
                        >
                          {t('sync.takeLocal')}
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={loading}
                          onClick={() => void resolve(c.id, 'remote')}
                        >
                          {t('sync.takeRemote')}
                        </Button>
                      </div>
                    ) : (
                      <div className="sync-muted">
                        {t('sync.resolvedAs', { resolution: c.resolution ?? '' })}
                      </div>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))
        )}
      </section>
    </div>
  );
}
