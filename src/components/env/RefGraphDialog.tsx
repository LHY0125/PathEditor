import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend, type RefEdge } from '@/services/backend';
import { Modal } from '@/components/ui/Modal';

/**
 * `%VAR%` 引用图对话框（Task 11）。
 *
 * **三个方向都要展示**（开工提示词 §五 陷阱二）：
 * - 反向（`refs_dependents`）：谁引用了它 → **删除前的影响范围**
 * - 前向（`refs_dependencies`）：它递归依赖了谁
 * - 环（`refs_cycles`）：循环引用 → 高亮
 *
 * 只做反向会让用户看不到「删掉它之后，它依赖的东西是否还有别人用」；
 * 只做前向则回答不了「删掉它会不会破坏别人」。两个方向回答的是不同问题。
 */

interface Props {
  /** 变量名（大写比较键由 Rust 侧处理） */
  name: string;
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

function describeError(e: unknown, fallback: string): string {
  if (isCoreError(e)) {
    if (e.code === 'database') return '引用图不可用（降级模式）';
    return e.message.trim().length > 0 ? e.message : fallback;
  }
  return e instanceof Error ? e.message : String(e);
}

export function RefGraphDialog({ name, onClose }: Props) {
  const { t } = useTranslation();
  const [dependents, setDependents] = useState<RefEdge[]>([]);
  const [dependencies, setDependencies] = useState<RefEdge[]>([]);
  const [cycles, setCycles] = useState<string[][]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    // 三个方向并行取——它们互不依赖，串行只会让对话框更慢
    void Promise.all([
      backend.refsDependents(name),
      backend.refsDependencies(name),
      backend.refsCycles(),
    ])
      .then(([dep, deps, cyc]) => {
        if (cancelled) return;
        setDependents(dep);
        setDependencies(deps);
        setCycles(cyc);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(describeError(e, t('refs.loadFailed')));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [name, t]);

  // 该变量参与的环（环里出现它才算相关）
  const relatedCycles = cycles.filter((c) => c.some((n) => n.toUpperCase() === name.toUpperCase()));

  return (
    <Modal open onClose={onClose}>
      <div className="p-4" style={{ minWidth: '26rem', maxWidth: '38rem' }}>
        <h3 className="mb-3 text-sm font-semibold" style={{ color: 'var(--app-fg)' }}>
          {t('refs.title', { name })}
        </h3>

        {loading && (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('refs.loading')}
          </p>
        )}

        {error !== null && (
          <p role="alert" className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        {!loading && error === null && (
          <>
            {/* 反向：删除前的影响范围 */}
            <section className="mb-3">
              <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
                {t('refs.dependentsTitle')}
              </h4>
              {dependents.length === 0 ? (
                <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
                  {t('refs.noDependents')}
                </p>
              ) : (
                <ul className="space-y-0.5 text-xs">
                  {dependents.map((e) => (
                    <li key={`${e.fromHive}-${e.fromName}`} style={{ color: 'var(--app-fg)' }}>
                      {e.fromHive}/{e.fromName}
                    </li>
                  ))}
                </ul>
              )}
            </section>

            {/* 前向：它依赖了谁（递归） */}
            <section className="mb-3">
              <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
                {t('refs.dependenciesTitle')}
              </h4>
              {dependencies.length === 0 ? (
                <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
                  {t('refs.noDependencies')}
                </p>
              ) : (
                <ul className="space-y-0.5 text-xs">
                  {dependencies.map((e) => (
                    <li
                      key={`${e.fromName}-${e.toName}-${e.depth}`}
                      style={{ color: 'var(--app-fg)', paddingLeft: `${(e.depth - 1) * 12}px` }}
                    >
                      {e.toName}
                      {e.depth > 1 && (
                        <span className="ml-1" style={{ color: 'var(--app-fg-faint)' }}>
                          ({t('refs.depth', { depth: e.depth })})
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </section>

            {/* 环：高亮 */}
            <section>
              <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
                {t('refs.cyclesTitle')}
              </h4>
              {relatedCycles.length === 0 ? (
                <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
                  {t('refs.noCycles')}
                </p>
              ) : (
                <ul className="space-y-0.5 text-xs">
                  {relatedCycles.map((c) => (
                    <li
                      key={c.join('>')}
                      className="rounded px-1"
                      style={{ background: 'var(--app-warning)', color: 'var(--app-accent-fg)' }}
                    >
                      {c.join(' → ')} → {c[0]}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </>
        )}

        <div className="mt-4 flex justify-end">
          <button type="button" className="tab-pill" onClick={onClose}>
            {t('refs.close')}
          </button>
        </div>
      </div>
    </Modal>
  );
}
