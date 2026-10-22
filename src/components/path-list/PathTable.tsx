import { useMemo, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { canWriteTarget, useAppStore } from '@/store/app-store';
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
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);

  const paths = tabId === 'system' ? sysPaths : userPaths;
  const isActive = activeTab === tabId;
  const target = tabId === 'system' ? TargetType.SYSTEM : TargetType.USER;
  const canWrite = canWriteTarget(isAdmin, pathCapabilities, target);

  const { validationCache, expandedCache } = usePathValidation(paths);

  // 搜索过滤
  const filtered = useMemo<PathRow[]>(() => {
    if (!searchQuery) return paths.map((p, i) => ({ path: p.path, index: i, enabled: p.enabled }));
    const q = searchQuery.toLowerCase();
    const result: PathRow[] = [];
    for (let i = 0; i < paths.length; i++) {
      const p = paths[i];
      if (p.path.toLowerCase().includes(q))
        result.push({ path: p.path, index: i, enabled: p.enabled });
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
      const state: ValidationState = validationCache.get(path) ?? 'pending';
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
    <div ref={parentRef} className="flex-1 overflow-auto relative" data-testid="path-table">
      <div className="path-table-head">
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

          // 行状态只走一条优先级链：禁用 > 无效 > 重复 > 正常。
          // 颜色由 globals.css 按 [data-state] 提供，组件内不再出现色值。
          const rowState = !enabled
            ? 'off'
            : v.state === 'invalid'
              ? 'invalid'
              : v.isDuplicate
                ? 'dup'
                : 'ok';

          return (
            <div
              key={virtualRow.key}
              data-testid="path-row"
              data-path={path}
              data-state={rowState}
              // 交替底色按逻辑索引决定 —— 虚拟滚动只渲染可视窗口，
              // :nth-child(odd) 的奇偶取决于「当前渲染了哪些节点」，滚动时会跳变闪烁。
              data-odd={rowIdx % 2 === 1}
              onClick={(e) => handleClick(index, e)}
              onDoubleClick={() => handleDoubleClick(index)}
              className={`path-row${isSelected ? ' is-selected' : ''}`}
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <div className="w-8 px-2 py-0.5 text-xs opacity-50">{index + 1}</div>
              <div className="w-6 px-1 py-0.5 flex items-center">
                <input
                  type="checkbox"
                  data-testid="path-enabled"
                  checked={enabled}
                  disabled={!canWrite}
                  onChange={() => {
                    useAppStore.getState().togglePath(index, target);
                  }}
                  className={canWrite ? 'cursor-pointer' : 'cursor-not-allowed'}
                />
              </div>
              <div
                data-testid="path-text"
                className="path-text"
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
