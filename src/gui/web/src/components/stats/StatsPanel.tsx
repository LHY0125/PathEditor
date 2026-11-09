import { useEffect, useState } from 'react';
import { isCoreError } from '@/core/error';
import { useTranslation } from 'react-i18next';
import { backend, type Dashboard, type DbHealth, type RetentionSettings } from '@/services/backend';
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
    <div className="stats-panel flex-1 min-h-0">
      {/* ── 当前状态（与历史序列口径不同，单独一块）── */}
      <section className="stats-card stats-card-wide">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
          {t('stats.currentTitle')}
        </h4>
        <div className="stats-kv">
          <div>
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.currentPathItems')}</span>
            <span className="stats-kv-value">{data.currentPathItems}</span>
          </div>
          <div>
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.currentAvgLen')}</span>
            <span className="stats-kv-value">{data.currentAvgLen.toFixed(1)}</span>
          </div>
        </div>
        <p className="mt-1 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('stats.currentHint')}
        </p>
      </section>

      {/* ── 数据库健康（#7）：自检入口，异常时用警告色突出 ── */}
      {health !== null && (
        <section className="stats-card stats-card-wide">
          <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
            {t('stats.healthTitle')}
          </h4>
          <div className="stats-kv">
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
          <div className="mt-1 flex items-center gap-2">
            <p
              className="min-w-0 flex-1 truncate text-xs"
              style={{ color: 'var(--app-fg-muted)' }}
              title={health.path}
            >
              {health.path}
            </p>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void backend.openDataDir().catch(() => {})}
            >
              {t('stats.healthOpenDir')}
            </Button>
          </div>
        </section>
      )}

      {/* ── 历史序列 ── */}
      <section className="stats-card stats-card-tall">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
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
      <section className="stats-card">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
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
      <section className="stats-card">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
          {t('stats.conflictTop')}
        </h4>
        <p className="mb-1 text-xs" style={{ color: 'var(--app-fg)' }}>
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
      <RetentionCard />
      <section className="stats-card stats-card-span2">
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
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

/** 保留策略卡片：数值写入 config.ini，下次 prune 生效。 */
function RetentionCard() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<RetentionSettings | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void backend
      .retentionSettings()
      .then(setSettings)
      .catch(() => setSettings(null));
  }, []);

  if (settings === null) return null;

  const update = (patch: Partial<RetentionSettings>) => {
    setSettings((current) => (current ? { ...current, ...patch } : current));
    setStatus(null);
    setError(null);
  };

  const save = async () => {
    setSaving(true);
    try {
      await backend.retentionSave(settings);
      setStatus(t('stats.retentionSaved'));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus(null);
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="stats-card stats-card-wide">
      <h4 className="mb-2 text-xs font-semibold" style={{ color: 'var(--app-fg)' }}>
        {t('stats.retentionTitle')}
      </h4>
      <div className="flex flex-wrap items-end gap-3 text-xs">
        <label className="flex flex-col gap-1">
          <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.retentionSnapshotDays')}</span>
          <input
            type="number"
            min={1}
            className="field-input w-28"
            value={settings.snapshotKeepDays}
            onChange={(e) => update({ snapshotKeepDays: Number(e.target.value) })}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.retentionSnapshotMax')}</span>
          <input
            type="number"
            min={1}
            className="field-input w-28"
            value={settings.snapshotKeepMax}
            onChange={(e) => update({ snapshotKeepMax: Number(e.target.value) })}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span style={{ color: 'var(--app-fg-muted)' }}>{t('stats.retentionEventDays')}</span>
          <input
            type="number"
            min={1}
            className="field-input w-28"
            value={settings.eventKeepDays}
            onChange={(e) => update({ eventKeepDays: Number(e.target.value) })}
          />
        </label>
        <Button size="sm" disabled={saving} onClick={() => void save()}>
          {t('stats.retentionSave')}
        </Button>
        {status !== null && <span style={{ color: 'var(--app-success)' }}>{status}</span>}
        {error !== null && <span style={{ color: 'var(--app-danger)' }}>{error}</span>}
      </div>
    </section>
  );
}
