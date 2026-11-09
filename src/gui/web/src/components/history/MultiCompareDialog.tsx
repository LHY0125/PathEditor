import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { backend, type MultiCompareEntry } from '@/services/backend';
import { errorMessage } from '@/core/error';

interface Props {
  uids: string[];
  onClose: () => void;
}

/** N 路快照对比：第一份为基准，其余逐份给出差异计数。 */
export function MultiCompareDialog({ uids, onClose }: Props) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<MultiCompareEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void backend
      .compareMany(uids)
      .then((value) => {
        if (!cancelled) setRows(value);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  }, [uids]);

  return (
    <Modal open onClose={onClose} ariaLabel={t('compare.manyTitle')}>
      <div className="p-4" style={{ width: 720, maxWidth: '100%' }}>
        <h3 className="mb-3 text-sm font-semibold">{t('compare.manyTitle')}</h3>
        {error !== null && (
          <p role="alert" className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}
        {rows === null && error === null && (
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('compare.loading')}
          </p>
        )}
        {rows !== null && (
          <div className="space-y-2 text-xs">
            <p style={{ color: 'var(--app-fg-muted)' }}>
              {t('compare.base', { uid: uids[0]?.slice(0, 8) ?? '' })}
            </p>
            {rows.map((row, index) => (
              <div
                key={row.otherUid}
                className="rounded border p-2"
                style={{ borderColor: 'var(--app-border)' }}
              >
                <div className="font-medium">
                  #{index + 2} {row.otherUid.slice(0, 8)}
                </div>
                <div style={{ color: 'var(--app-fg-muted)' }}>
                  {t('compare.pathsSummary', {
                    sysAdded: row.diff.system.added.length,
                    sysRemoved: row.diff.system.removed.length,
                    userAdded: row.diff.user.added.length,
                    userRemoved: row.diff.user.removed.length,
                  })}
                </div>
                <div style={{ color: 'var(--app-fg-muted)' }}>
                  {t('compare.envSummary', {
                    added: row.diff.envAdded.join(', ') || '—',
                    removed: row.diff.envRemoved.join(', ') || '—',
                    modified: row.diff.envModified.join(', ') || '—',
                  })}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </Modal>
  );
}
