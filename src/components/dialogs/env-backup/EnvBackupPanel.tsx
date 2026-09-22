import { useTranslation } from 'react-i18next';
import { formatSize } from '@/core/env-backup';
import { btnClass, btnStyle } from '@/components/ui/buttons';
import type { EnvBackupController } from './use-env-backup';

interface EnvBackupPanelProps {
  controller: EnvBackupController;
}

const sectionTitleClass = 'text-xs font-semibold uppercase tracking-wide opacity-60';

/**
 * 备份与恢复对话框主体。
 *
 * 差异摘要**如实展示四个计数，不做任何换算**：`modified` 恒为 0（Rust 侧
 * 「值变了」≡「备份过期」，一律计 conflict），强制覆盖低报改动量 —— 两个
 * 取舍都在界面上写明，而不是悄悄修正数字。
 */
export function EnvBackupPanel({ controller }: EnvBackupPanelProps) {
  const { t } = useTranslation();
  const { backups, busy, creating, selected, summary, preview, outcome, createdPath, error } =
    controller;

  return (
    <div className="flex flex-col gap-3 text-sm" style={{ width: 720, maxHeight: '78vh' }}>
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">{t('envBackup.title')}</h2>
        <button
          className={btnClass}
          style={btnStyle}
          disabled={creating}
          onClick={() => void controller.createBackup()}
        >
          {creating ? t('envBackup.creating') : t('envBackup.create')}
        </button>
      </div>

      {createdPath && (
        <p className="text-green-600 text-xs break-all">
          {t('envBackup.created', { path: createdPath })}
        </p>
      )}

      <div className="flex gap-3 overflow-hidden" style={{ minHeight: 260 }}>
        {/* 左：备份文件列表（Rust 侧已按时间倒序，前端不再排序） */}
        <div
          className="w-72 flex flex-col rounded border overflow-auto"
          style={{ borderColor: 'var(--app-border)', backgroundColor: 'var(--app-list-bg)' }}
        >
          <div className={sectionTitleClass + ' px-3 py-2'}>{t('envBackup.title')}</div>
          {backups === null ? (
            <p className="px-3 py-2 text-xs opacity-60">{t('envBackup.loading')}</p>
          ) : backups.length === 0 ? (
            <p className="px-3 py-2 text-xs opacity-60">{t('envBackup.empty')}</p>
          ) : (
            backups.map((info) => (
              <button
                key={info.file}
                data-backup-file={info.file}
                className={`px-3 py-2 text-left text-xs border-b transition-colors ${
                  selected?.file === info.file ? 'tab-active' : 'hover:opacity-80'
                }`}
                style={{ borderColor: 'var(--app-border)' }}
                disabled={busy}
                onClick={() => void controller.select(info)}
              >
                <div className="font-mono truncate" title={info.file}>
                  {info.file}
                </div>
                <div className="opacity-60">
                  {info.timestamp} · {formatSize(info.sizeBytes)}
                </div>
              </button>
            ))
          )}
        </div>

        {/* 右：差异预览与恢复 */}
        <div className="flex-1 flex flex-col gap-2 overflow-auto">
          {selected === null ? (
            <p className="text-xs opacity-60">{t('envBackup.selectHint')}</p>
          ) : busy && preview === null ? (
            <p className="text-xs opacity-60">{t('envBackup.previewing')}</p>
          ) : preview === null ? (
            <p className="text-xs opacity-60">{t('envBackup.noPreview')}</p>
          ) : (
            <>
              <div className={sectionTitleClass}>{t('envBackup.preview')}</div>
              <p data-testid="backup-summary" className="text-xs">
                {summary?.text}
              </p>
              <div className="grid grid-cols-4 gap-2 text-xs">
                <Count label={t('envBackup.addedLabel')} value={preview.added} />
                <Count label={t('envBackup.modifiedLabel')} value={preview.modified} />
                <Count label={t('envBackup.removedLabel')} value={preview.removed} />
                <Count label={t('envBackup.conflictsLabel')} value={preview.conflicts} />
              </div>
              {preview.removed > 0 && (
                <div
                  data-testid="backup-removed-names"
                  className="rounded border p-2 text-xs"
                  style={{ borderColor: '#dc2626' }}
                >
                  <p className="text-red-500 font-semibold">{t('envBackup.deleteWarning')}</p>
                  <ul className="font-mono mt-1">
                    {(summary?.removedNames ?? []).map((name) => (
                      <li key={name}>• {name}</li>
                    ))}
                  </ul>
                  <p className="mt-1 opacity-70">{t('envBackup.deleteWarningHint')}</p>
                </div>
              )}
              {preview.modified === 0 && (
                <p className="text-xs opacity-60">{t('envBackup.modifiedAlwaysZero')}</p>
              )}
              {preview.conflicts > 0 && (
                <p className="text-xs opacity-60">{t('envBackup.forceUnderreport')}</p>
              )}
              <p className="text-xs opacity-70">{t('envBackup.manualFallback')}</p>
              <p className="text-xs font-mono opacity-70">patheditor backup</p>
              <p className="text-xs font-mono opacity-70">patheditor env backup</p>
              {!summary?.hasChanges && !summary?.hasConflicts && (
                <p className="text-xs opacity-60">{t('envBackup.noDiffHint')}</p>
              )}
              <button
                className={btnClass}
                style={{ ...btnStyle, backgroundColor: '#2563eb', color: '#fff' }}
                disabled={busy}
                onClick={() => void controller.restore()}
              >
                {busy ? t('envBackup.restoring') : t('envBackup.restore')}
              </button>
            </>
          )}

          {outcome !== null && (
            <div data-testid="backup-outcome" className="rounded border p-2 text-xs">
              <p>
                {outcome.failures.length === 0
                  ? t('envBackup.restoreDoneAll', { applied: outcome.applied })
                  : t('envBackup.restoreDone', {
                      applied: outcome.applied,
                      failed: outcome.failures.length,
                    })}
              </p>
              {outcome.failures.length > 0 && (
                <>
                  <p className="text-red-500 mt-1">{t('envBackup.failedTitle')}</p>
                  <ul className="font-mono">
                    {outcome.failures.map((failure) => (
                      <li key={failure}>{failure}</li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          )}

          {error && <p className="text-red-500 text-xs">{error}</p>}
        </div>
      </div>
    </div>
  );
}

/** 单个差异计数。 */
function Count({ label, value }: { label: string; value: number }) {
  return (
    <div
      className="rounded border px-2 py-1"
      style={{ borderColor: 'var(--app-border)', backgroundColor: 'var(--app-list-bg)' }}
    >
      <div className="opacity-60">{label}</div>
      <div className="font-semibold">{value}</div>
    </div>
  );
}
