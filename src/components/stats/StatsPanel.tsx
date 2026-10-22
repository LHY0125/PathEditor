import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend, type Dashboard } from '@/services/backend';

/**
 * 统计面板。
 *
 * **口径必须分开显示**：`days` 是按天的**历史**序列，
 * `currentPathItems` / `currentAvgLen` 是**现在**的状态。
 * 混在一起显示会让用户把「今天最后一次保存」误读成「当前」。
 *
 * **不引入图表库**：用 CSS 宽度画横向条形。依赖里没有任何图表库，
 * 为几条柱状图引入一个是过度设计（YAGNI），且与「不引入新依赖」的约束冲突。
 */

function isCoreError(err: unknown): err is { code: string; message: string } {
  return (
    typeof err === 'object' &&
    err !== null &&
    typeof (err as { code?: unknown }).code === 'string' &&
    typeof (err as { message?: unknown }).message === 'string'
  );
}

function errorMessage(err: unknown): string {
  if (isCoreError(err)) {
    if (err.code === 'database') return '统计不可用（降级模式）';
    return err.message.trim().length > 0 ? err.message : '统计加载失败';
  }
  return err instanceof Error ? err.message : String(err);
}

export function StatsPanel() {
  const { t } = useTranslation();
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void backend
      .statsDashboard(30)
      .then((d) => {
        if (!cancelled) {
          setData(d);
          setError(null);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(errorMessage(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) {
    return (
      <p className="p-3 text-xs" style={{ color: 'var(--app-fg-faint)' }}>
        {t('stats.loading')}
      </p>
    );
  }

  if (error !== null) {
    return (
      <p role="alert" className="p-3 text-xs" style={{ color: 'var(--app-danger)' }}>
        {error}
      </p>
    );
  }

  if (data === null) return null;

  // 条形图的最大值：至少为 1，避免除零；用历史序列里的最大事件数做标尺
  const maxEvents = Math.max(1, ...data.days.map((d) => d.events));

  return (
    <div className="p-3">
      {/* ── 当前状态（与历史序列口径不同，单独一块）── */}
      <section className="mb-4">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.currentTitle')}
        </h4>
        <div className="flex gap-4 text-xs">
          <div>
            <span style={{ color: 'var(--app-fg-faint)' }}>{t('stats.currentPathItems')}</span>
            <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
              {data.currentPathItems}
            </span>
          </div>
          <div>
            <span style={{ color: 'var(--app-fg-faint)' }}>{t('stats.currentAvgLen')}</span>
            <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
              {data.currentAvgLen.toFixed(1)}
            </span>
          </div>
        </div>
        <p className="mt-1 text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {t('stats.currentHint')}
        </p>
      </section>

      {/* ── 历史序列 ── */}
      <section className="mb-4">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.historyTitle')}
        </h4>
        {data.days.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('stats.noData')}
          </p>
        ) : (
          <ul className="space-y-1" aria-label={t('stats.historyTitle')}>
            {data.days.map((d) => (
              <li key={d.day} className="flex items-center gap-2 text-xs">
                <span className="w-24 shrink-0" style={{ color: 'var(--app-fg-faint)' }}>
                  {d.day}
                </span>
                <span
                  className="h-2 shrink-0 rounded"
                  style={{
                    width: `${Math.max(2, (d.events / maxEvents) * 120)}px`,
                    background: 'var(--app-accent)',
                  }}
                  aria-hidden="true"
                />
                <span style={{ color: 'var(--app-fg-muted)' }}>
                  {t('stats.eventsCount', { count: d.events })}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* ── 最常改变的变量 ── */}
      <section className="mb-4">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.topTargets')}
        </h4>
        {data.topTargets.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('stats.noData')}
          </p>
        ) : (
          <ul className="space-y-0.5 text-xs">
            {data.topTargets.map((tgt) => (
              <li key={tgt.target} className="flex justify-between">
                <span style={{ color: 'var(--app-fg)' }}>{tgt.target}</span>
                <span style={{ color: 'var(--app-fg-muted)' }}>{tgt.count}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* ── 配置应用次数 ── */}
      <section>
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.profileApplies')}
        </h4>
        {data.profileApplies.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('stats.noData')}
          </p>
        ) : (
          <ul className="space-y-0.5 text-xs">
            {data.profileApplies.map((p) => (
              <li key={p.name} className="flex justify-between">
                <span style={{ color: 'var(--app-fg)' }}>{p.name}</span>
                <span style={{ color: 'var(--app-fg-muted)' }}>{p.applyCount}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
