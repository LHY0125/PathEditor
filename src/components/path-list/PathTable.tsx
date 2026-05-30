import { useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
import { TargetType } from '@/core/undo-redo';
import { usePathValidation } from '@/hooks/use-path-validation';
import type { ValidationState } from '@/hooks/use-path-validation';

interface PathTableProps {
  tabId: 'system' | 'user';
}

interface PathRow {
  path: string;
  index: number;
  enabled: boolean;
}

export function PathTable({ tabId }: PathTableProps) {
  const { t } = useTranslation();
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const selectedIndices = useAppStore((s) => s.selectedIndices);
  const setSelectedIndices = useAppStore((s) => s.setSelectedIndices);
  const activeTab = useAppStore((s) => s.activeTab);

  const paths = tabId === 'system' ? sysPaths : userPaths;
  const isActive = activeTab === tabId;

  const { validationCache, expandedCache } = usePathValidation(paths);

  // 搜索过滤
  const filtered = useMemo<PathRow[]>(() => {
    if (!searchQuery) return paths.map((p, i) => ({ path: p.path, index: i, enabled: p.enabled }));
    const q = searchQuery.toLowerCase();
    const result: PathRow[] = [];
    for (let i = 0; i < paths.length; i++) {
      const p = paths[i];
      if (p.path.toLowerCase().includes(q)) result.push({ path: p.path, index: i, enabled: p.enabled });
    }
    return result;
  }, [paths, searchQuery]);

  // 计算验证状态（含去重检测）
  const validations = useMemo(() => {
    const seen = new Set<string>();
    return filtered.map(({ path }) => {
      const lower = path.toLowerCase();
      const isDuplicate = seen.has(lower);
      seen.add(lower);
      const state: ValidationState = validationCache.get(path) ?? 'valid';
      return { state, isDuplicate, isEnvVar: path.includes('%') };
    });
  }, [filtered, validationCache]);

  const handleClick = useCallback(
    (realIndex: number, e: React.MouseEvent) => {
      if (!isActive) return;
      if (e.ctrlKey) {
        const next = selectedIndices.includes(realIndex)
          ? selectedIndices.filter((i) => i !== realIndex)
          : [...selectedIndices, realIndex];
        setSelectedIndices(next);
      } else {
        setSelectedIndices([realIndex]);
      }
    },
    [isActive, selectedIndices, setSelectedIndices],
  );

  const handleDoubleClick = useCallback(
    (realIndex: number) => {
      if (!isActive) return;
      window.dispatchEvent(
        new CustomEvent('path-dblclick', {
          detail: { index: realIndex, path: paths[realIndex].path },
        }),
      );
    },
    [isActive, paths],
  );

  return (
    <div className="flex-1 overflow-auto">
      <table className="w-full border-collapse">
        <thead>
          <tr
            className="sticky top-0 z-10 text-left text-xs uppercase"
            style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
          >
            <th className="w-8 px-2 py-1">#</th>
            <th className="w-6 px-1 py-1"></th>
            <th className="px-2 py-1">{t('table.path')}</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map(({ path, index, enabled }, rowIdx) => {
            const v = validations[rowIdx];
            const isSelected = selectedIndices.includes(index);
            let textColor = 'var(--app-fg)';
            if (v.state === 'invalid') textColor = '#dc3545';
            else if (v.isDuplicate) textColor = '#fd7e14';
            else if (v.state === 'unknown') textColor = 'var(--app-fg)';

            let textDecoration = 'none';
            let opacity = 1;
            if (!enabled) {
              textColor = '#6b7280';
              textDecoration = 'line-through';
              opacity = 0.6;
            }

            return (
              <tr
                key={index}
                onClick={(e) => handleClick(index, e)}
                onDoubleClick={() => handleDoubleClick(index)}
                className="cursor-pointer select-none"
                style={{
                  backgroundColor: isSelected
                    ? 'var(--app-select-row)'
                    : rowIdx % 2 === 0
                      ? 'var(--app-list-bg)'
                      : 'var(--app-list-alt)',
                }}
              >
                <td className="w-8 px-2 py-0.5 text-xs opacity-50" style={{ color: 'var(--app-fg)' }}>
                  {index + 1}
                </td>
                <td className="w-6 px-1 py-0.5">
                  <input
                    type="checkbox"
                    checked={enabled}
                    onChange={() => {
                      const target = tabId === 'system' ? TargetType.SYSTEM : TargetType.USER;
                      useAppStore.getState().togglePath(index, target);
                    }}
                    className="cursor-pointer"
                  />
                </td>
                <td
                  className="px-2 py-0.5 text-sm truncate max-w-2xl"
                  style={{ color: textColor, textDecoration, opacity }}
                  title={expandedCache.get(path) || undefined}
                >
                  {path}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
