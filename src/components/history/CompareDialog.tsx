import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend, type CompareResult, type PathDiff } from '@/services/backend';
import { Modal } from '@/components/ui/Modal';

/**
 * 快照对比对话框（Task 11）。
 *
 * **双 hive 分开显示**（N-8）：`system` 与 `user` 各一块。
 * 只显示一侧会让「只有用户 PATH 变了」看起来像「没有差异」。
 *
 * **启用状态变化也要显示**（`toggled`）：`PathEntry { path, enabled }`
 * 是跨层契约，同一条路径从启用变禁用是真实变化。计划的 `compare_paths`
 * 只比路径串，本实现在 core 侧补了这个维度，UI 必须一并呈现。
 *
 * **env 对比恒为空**（T-3）：显式显示「将在 M3 提供」，
 * 而不是让用户以为「环境变量没变化」。
 */

interface Props {
  leftUid: string;
  rightUid: string;
  onClose: () => void;
}

function isCoreError(err: unknown): err is { code: string; message: string } {
  return (
    typeof err === 'object' &&
    err !== null &&
    typeof (err as { code?: unknown }).code === 'string' &&
    typeof (err as { message?: unknown }).message === 'string'
  );
}

/** 单侧差异块。 */
function DiffSection({ title, diff }: { title: string; diff: PathDiff }) {
  const { t } = useTranslation();
  const empty =
    diff.added.length === 0 &&
    diff.removed.length === 0 &&
    diff.reordered.length === 0 &&
    diff.toggled.length === 0;

  return (
    <section className="mb-4">
      <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
        {title}
      </h4>
      {empty ? (
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {t('compare.noChange')}
        </p>
      ) : (
        <ul className="space-y-0.5 text-xs">
          {diff.added.map((e) => (
            <li key={`add-${e.path}`} style={{ color: 'var(--app-success)' }}>
              + {e.path}
            </li>
          ))}
          {diff.removed.map((e) => (
            <li key={`del-${e.path}`} style={{ color: 'var(--app-danger)' }}>
              − {e.path}
            </li>
          ))}
          {diff.reordered.map((r) => (
            <li key={`ord-${r.path}`} style={{ color: 'var(--app-warning)' }}>
              {t('compare.moved', { path: r.path, from: r.leftIndex + 1, to: r.rightIndex + 1 })}
            </li>
          ))}
          {diff.toggled.map((tg) => (
            <li key={`tog-${tg.path}`} style={{ color: 'var(--app-accent-text)' }}>
              {t('compare.toggled', {
                path: tg.path,
                state: tg.rightEnabled ? t('compare.enabled') : t('compare.disabled'),
              })}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function CompareDialog({ leftUid, rightUid, onClose }: Props) {
  const { t } = useTranslation();
  const [result, setResult] = useState<CompareResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void backend
      .compareSnapshots(leftUid, rightUid)
      .then((r) => {
        if (!cancelled) {
          setResult(r);
          setError(null);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setError(isCoreError(e) ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [leftUid, rightUid]);

  return (
    <Modal open onClose={onClose}>
      <div className="p-4" style={{ minWidth: '28rem', maxWidth: '40rem' }}>
        <h3 className="mb-3 text-sm font-semibold" style={{ color: 'var(--app-fg)' }}>
          {t('compare.title')}
        </h3>

        {loading && (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('compare.loading')}
          </p>
        )}

        {error !== null && (
          <p role="alert" className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        {result !== null && (
          <>
            <DiffSection title={t('history.systemPath')} diff={result.system} />
            <DiffSection title={t('history.userPath')} diff={result.user} />

            {/* T-3：env 对比未实现，必须显式说明而不是留白 */}
            <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
              {t('compare.envNotAvailable')}
            </p>
          </>
        )}

        <div className="mt-4 flex justify-end">
          <button type="button" className="tab-pill" onClick={onClose}>
            {t('compare.close')}
          </button>
        </div>
      </div>
    </Modal>
  );
}
