import { useEffect, useState } from 'react';
import { isCoreError } from '@/core/error';
import { useTranslation } from 'react-i18next';
import { backend, type Dashboard, type DbHealth } from '@/services/backend';
import { Button } from '@/components/ui/Button';

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

function errorMessage(err: unknown, t: (key: string) => string): string {
  if (isCoreError(err)) {
    if (err.code === 'database') return t('stats.errorDegraded');
    return err.message.trim().length > 0 ? err.message : t('stats.errorLoadFailed');
  }
  return err instanceof Error ? err.message : String(err);
}

export function StatsPanel() {
  const { t } = useTranslation();
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [reloadKey, setReloadKey] = useState(0);
  const [health, setHealth] = useState<DbHealth | null>(null);

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
        if (!cancelled) setError(errorMessage(e, t));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [reloadKey, t]);

  useEffect(() => {
    let cancelled = false;
    void backend
      .dbHealth()
      .then((h) => {
        if (!cancelled) setHealth(h);
      })
      .catch(() => {
        // 健康信息是附加信息：取不到不影响统计主视图
        if (!cancelled) setHealth(null);
      });
    return () => {
      cancelled = true;
    };
  }, [reloadKey]);

  if (loading) {
    return (
      <p className="p-3 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
        {t('stats.loading')}
      </p>
    );
  }

  if (error !== null) {
    return (
      <div role="alert" className="p-3 text-xs">
        <p style={{ color: 'var(--app-danger)' }}>{error}</p>
        <Button
          size="sm"
          className="mt-2"
          onClick={() => {
            setLoading(true);
            setReloadKey((k) => k + 1);
          }}
        >
          {t('stats.retry')}
        </Button>
      </div>
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
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.currentPathItems')}</span>
            <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
              {data.currentPathItems}
            </span>
          </div>
          <div>
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.currentAvgLen')}</span>
            <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
              {data.currentAvgLen.toFixed(1)}
            </span>
          </div>
        </div>
        <p className="mt-1 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.currentHint')}
        </p>
      </section>

      {/* ── 数据库健康（#7）：自检入口，异常时用警告色突出 ── */}
      {health !== null && (
        <section className="mb-4">
          <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
            {t('stats.healthTitle')}
          </h4>
          <div className="flex flex-wrap gap-4 text-xs">
            <div>
              <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.healthSchema')}</span>
              <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
                v{health.schemaVersion}
              </span>
            </div>
            <div style={{ color: health.integrityOk ? 'var(--app-success)' : 'var(--app-danger)' }}>
              {health.integrityOk
                ? t('stats.healthOk')
                : `${t('stats.healthBad')}: ${health.integrity}`}
            </div>
            <div>
              <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.healthSize')}</span>
              <span className="ml-1 font-semibold" style={{ color: 'var(--app-fg)' }}>
                {((health.fileBytes + health.walBytes) / (1024 * 1024)).toFixed(2)} MiB
              </span>
            </div>
            <div>
              <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.healthLastPrune')}</span>
              <span className="ml-1" style={{ color: 'var(--app-fg)' }}>
                {health.lastPruneAt === null
                  ? t('stats.healthNever')
                  : new Date(health.lastPruneAt).toLocaleString()}
              </span>
            </div>
          </div>
          <p
            className="mt-1 truncate text-xs"
            style={{ color: 'var(--app-fg-muted)' }}
            title={health.path}
          >
            {health.path}
          </p>
        </section>
      )}

      {/* ── 历史序列 ── */}
      <section className="mb-4">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.historyTitle')}
        </h4>
        {data.days.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('stats.noData')}
          </p>
        ) : (
          <ul className="space-y-1" aria-label={t('stats.historyTitle')}>
            {data.days.map((d) => (
              <li key={d.day} className="flex items-center gap-2 text-xs">
                <span className="w-24 shrink-0" style={{ color: 'var(--app-fg-muted)' }}>
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
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
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

      {/* ── 冲突 Top（来自最近 200 次扫描） ── */}
      <section className="mb-4">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.conflictTop')}
        </h4>
        <p className="mb-1 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.scanSummary', {
            runs: data.scanRuns,
            rate: Math.round(data.conflictRate * 100),
          })}
        </p>
        {data.topConflicts.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('stats.noData')}
          </p>
        ) : (
          <ul className="space-y-0.5 text-xs">
            {data.topConflicts.map((item) => (
              <li key={item.target} className="flex justify-between">
                <span style={{ color: 'var(--app-fg)' }}>{item.target}</span>
                <span style={{ color: 'var(--app-fg-muted)' }}>{item.count}</span>
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
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
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
