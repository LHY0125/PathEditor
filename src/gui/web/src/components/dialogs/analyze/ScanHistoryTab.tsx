import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';
import type { ScanRunDiff, ScanRunView } from '@/services/backend';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/core/error';

/**
 * 扫描历史页签：列出最近扫描，选两次查看冲突差异。
 *
 * 数据来自 DB 的 `scan_runs`；无历史库时后端返回错误，这里按普通错误展示。
 */
export function ScanHistoryTab() {
  const { t } = useTranslation();
  const [runs, setRuns] = useState<ScanRunView[] | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [diff, setDiff] = useState<ScanRunDiff | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setRuns(await backend.scanHistory(20));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    // 首次加载不经过 `load()`：在 effect 体内同步 setState 会触发级联渲染
    // （React 的 effect 规范），这里只在 promise 回调里落地结果。
    let cancelled = false;
    backend.scanHistory(20).then(
      (runs) => {
        if (!cancelled) setRuns(runs);
      },
      (err) => {
        if (!cancelled) setError(errorMessage(err));
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = (uid: string) => {
    setDiff(null);
    setSelected((prev) => {
      if (prev.includes(uid)) return prev.filter((item) => item !== uid);
      if (prev.length >= 2) return prev;
      return [...prev, uid];
    });
  };

  const compare = async () => {
    if (selected.length !== 2) return;
    setBusy(true);
    setError(null);
    try {
      setDiff(await backend.scanDiff(selected[0], selected[1]));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const group = (label: string, items: string[], color: string) => (
    <div className="text-xs">
      <span className="font-semibold" style={{ color }}>
        {label}（{items.length}）
      </span>
      {items.length > 0 && (
        <ul className="mt-1 ml-3">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      )}
    </div>
  );

  if (error) {
    return (
      <div className="py-8 text-center text-xs" style={{ color: 'var(--app-danger)' }}>
        {error}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <span className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('analyze.historyHint')}
        </span>
        <div className="flex gap-1">
          <Button variant="ghost" disabled={busy} onClick={() => void load()}>
            {t('analyze.refresh')}
          </Button>
          <Button
            variant="ghost"
            disabled={busy || selected.length !== 2}
            onClick={() => void compare()}
          >
            {t('analyze.compareRuns', { count: selected.length })}
          </Button>
        </div>
      </div>

      {runs === null ? (
        <div className="py-8 text-center text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('analyze.scanning')}
        </div>
      ) : runs.length === 0 ? (
        <div className="py-8 text-center text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('analyze.historyEmpty')}
        </div>
      ) : (
        <ul className="flex flex-col gap-1">
          {runs.map((run) => (
            <li key={run.uid}>
              <button
                type="button"
                className="tab-pill w-full justify-start text-xs"
                data-active={selected.includes(run.uid)}
                onClick={() => toggle(run.uid)}
              >
                {new Date(run.createdAt).toLocaleString()} — 冲突 {run.conflicts.length} 个 /
                工具目录 {run.toolCount} 个
              </button>
            </li>
          ))}
        </ul>
      )}

      {diff && (
        <div
          className="flex flex-col gap-2 rounded border p-2"
          style={{ borderColor: 'var(--app-border)' }}
        >
          {group(t('analyze.appeared'), diff.appeared, 'var(--app-danger)')}
          {group(t('analyze.disappeared'), diff.disappeared, 'var(--app-success)')}
          {group(t('analyze.persistent'), diff.persistent, 'var(--app-fg-muted)')}
        </div>
      )}
    </div>
  );
}
