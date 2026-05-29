import { useState, useEffect, useMemo, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { useAppStore } from '@/store/app-store';

interface ConflictLocation {
  dir: string;
  priority: number;
}

interface ConflictEntry {
  name: string;
  locations: ConflictLocation[];
}

interface ToolGroup {
  dir: string;
  exists: boolean;
  exes: string[];
}

type TabType = 'conflicts' | 'tools';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AnalyzeDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<TabType>('conflicts');
  const [loading, setLoading] = useState(false);
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  const [toolGroups, setToolGroups] = useState<ToolGroup[]>([]);
  const [searchQuery, setSearchQuery] = useState('');

  const prevOpen = useRef(false);
  useEffect(() => {
    if (!open || prevOpen.current) return;
    prevOpen.current = open;
    setLoading(true);
    const paths = getEnabledPaths();
    Promise.all([
      invoke<ConflictEntry[]>('scan_conflicts', { paths }),
      invoke<ToolGroup[]>('scan_tools', { paths, query: '' }),
    ])
      .then(([c, t]) => {
        setConflicts(c);
        setToolGroups(t);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [open]);

  // 搜索的工具清单
  const filteredTools = useMemo(() => {
    if (!searchQuery.trim()) return toolGroups;
    const q = searchQuery.toLowerCase();
    return toolGroups
      .map((g) => ({ ...g, exes: g.exes.filter((e) => e.toLowerCase().includes(q)) }))
      .filter((g) => g.exes.length > 0);
  }, [toolGroups, searchQuery]);

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxHeight: '75vh' }}>
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-5 py-3 border-b" style={{ borderColor: 'var(--app-border)' }}>
          <h2 className="text-base font-semibold">{t('analyze.title')}</h2>
          <div className="flex gap-1">
            {(['conflicts', 'tools'] as TabType[]).map((tb) => (
              <button
                key={tb}
                onClick={() => setTab(tb)}
                className="px-3 py-1 text-sm rounded transition-colors"
                style={{
                  backgroundColor: tab === tb ? '#3b82f6' : 'transparent',
                  color: tab === tb ? '#fff' : 'var(--app-fg)',
                }}
              >
                {tb === 'conflicts' ? t('analyze.conflicts') : t('analyze.tools')}
              </button>
            ))}
          </div>
        </div>

        {/* 内容 */}
        <div className="flex-1 overflow-auto p-4">
          {loading ? (
            <div className="flex items-center justify-center py-12 text-sm" style={{ color: 'var(--app-fg)', opacity: 0.6 }}>
              {t('analyze.scanning')}
            </div>
          ) : tab === 'conflicts' ? (
            <ConflictsTab conflicts={conflicts} />
          ) : (
            <ToolsTab groups={filteredTools} query={searchQuery} onQueryChange={setSearchQuery} />
          )}
        </div>
      </div>
    </Modal>
  );
}

function ConflictsTab({ conflicts }: { conflicts: ConflictEntry[] }) {
  const { t } = useTranslation();
  if (conflicts.length === 0) {
    return <EmptyHint text={t('analyze.noConflicts')} />;
  }
  return (
    <div>
      <p className="text-sm mb-2" style={{ color: 'var(--app-fg)', opacity: 0.7 }}>
        {t('analyze.conflictCount', { count: conflicts.length })}
      </p>
      <table className="w-full text-sm border-collapse">
        <thead>
          <tr className="border-b" style={{ borderColor: 'var(--app-border)' }}>
            <th className="text-left py-1.5 pr-3 font-medium">EXE</th>
            <th className="text-left py-1.5 font-medium">{t('analyze.priority')}</th>
          </tr>
        </thead>
        <tbody>
          {conflicts.map((c) => (
            <tr key={c.name} className="border-b" style={{ borderColor: 'var(--app-border)' }}>
              <td className="py-1.5 pr-3 font-mono">{c.name}</td>
              <td className="py-1.5">
                {c.locations.map((loc, i) => (
                  <div
                    key={i}
                    className="text-xs py-0.5"
                    style={{ color: i === 0 ? '#22c55e' : '#ef4444' }}
                  >
                    {i === 0 ? '✓' : '✗'} {loc.dir}
                    {i > 0 && (
                      <span className="ml-1" style={{ opacity: 0.5 }}>
                        ({t('analyze.shadowed')})
                      </span>
                    )}
                  </div>
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ToolsTab({
  groups,
  query,
  onQueryChange,
}: {
  groups: ToolGroup[];
  query: string;
  onQueryChange: (q: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div>
      <input
        type="text"
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        placeholder={t('analyze.searchPlaceholder')}
        className="w-full px-3 py-1.5 text-sm rounded mb-3 border outline-none"
        style={{
          backgroundColor: 'var(--app-list-bg)',
          color: 'var(--app-fg)',
          borderColor: 'var(--app-border)',
        }}
      />
      {groups.length === 0 ? (
        <EmptyHint text={t('analyze.noTools')} />
      ) : (
        groups.map((g) => (
          <div key={g.dir} className="mb-3">
            <div
              className="text-xs font-mono py-1 px-2 rounded"
              style={{
                backgroundColor: g.exists ? 'transparent' : 'rgba(239,68,68,0.1)',
                color: g.exists ? 'var(--app-fg)' : '#ef4444',
                opacity: g.exists ? 1 : 0.6,
              }}
            >
              {g.dir} {!g.exists && '(不存在)'}
            </div>
            <div className="flex flex-wrap gap-1 mt-1 ml-2">
              {g.exes.map((exe) => (
                <span
                  key={exe}
                  className="text-xs font-mono px-1.5 py-0.5 rounded"
                  style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)' }}
                >
                  {exe}
                </span>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}

function EmptyHint({ text }: { text: string }) {
  return (
    <div className="text-center py-12 text-sm" style={{ color: 'var(--app-fg)', opacity: 0.5 }}>
      {text}
    </div>
  );
}

function getEnabledPaths(): string[] {
  const { sysPaths, userPaths } = useAppStore.getState();
  return [...sysPaths.filter((e) => e.enabled), ...userPaths.filter((e) => e.enabled)].map((e) => e.path);
}
