import { useMemo, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
import { TargetType } from '@/core/undo-redo';
import { usePathValidation } from '@/hooks/use-path-validation';
import type { ValidationState } from '@/hooks/use-path-validation';
import { useVirtualizer } from '@tanstack/react-virtual';

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

  const parentRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28, // 预估行高 28px
    initialRect: { width: 800, height: 600 },
  });

  return (
    <div ref={parentRef} className="flex-1 overflow-auto relative">
      <div
        className="sticky top-0 z-10 flex text-left text-xs uppercase"
        style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
      >
        <div className="w-8 px-2 py-1">#</div>
        <div className="w-6 px-1 py-1"></div>
        <div className="px-2 py-1 flex-1">{t('table.path')}</div>
      </div>
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          width: '100%',
          position: 'relative',
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow) => {
          const rowIdx = virtualRow.index;
          const { path, index, enabled } = filtered[rowIdx];
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
            <div
              key={virtualRow.key}
              onClick={(e) => handleClick(index, e)}
              onDoubleClick={() => handleDoubleClick(index)}
              className="cursor-pointer select-none flex items-center absolute top-0 left-0 w-full"
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
                backgroundColor: isSelected
                  ? 'var(--app-select-row)'
                  : rowIdx % 2 === 0
                    ? 'var(--app-list-bg)'
                    : 'var(--app-list-alt)',
              }}
            >
              <div className="w-8 px-2 py-0.5 text-xs opacity-50" style={{ color: 'var(--app-fg)' }}>
                {index + 1}
              </div>
              <div className="w-6 px-1 py-0.5 flex items-center">
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={() => {
                    const target = tabId === 'system' ? TargetType.SYSTEM : TargetType.USER;
                    useAppStore.getState().togglePath(index, target);
                  }}
                  className="cursor-pointer"
                />
              </div>
              <div
                className="px-2 py-0.5 text-sm truncate flex-1"
                style={{ color: textColor, textDecoration, opacity }}
                title={expandedCache.get(path) || undefined}
              >
                {path}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
