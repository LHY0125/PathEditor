import { useMemo, useCallback } from 'react';
import { useAppStore } from '@/store/app-store';
import { validatePath } from '@/hooks/use-path-validation';

interface PathTableProps {
  tabId: 'system' | 'user';
}

export function PathTable({ tabId }: PathTableProps) {
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const selectedIndices = useAppStore((s) => s.selectedIndices);
  const setSelectedIndices = useAppStore((s) => s.setSelectedIndices);
  const activeTab = useAppStore((s) => s.activeTab);

  const paths = tabId === 'system' ? sysPaths : userPaths;
  const isActive = activeTab === tabId;

  // 过滤搜索
  const filtered = useMemo(() => {
    if (!searchQuery) return paths.all.map((p, i) => ({ path: p, index: i }));
    const q = searchQuery.toLowerCase();
    const result: { path: string; index: number }[] = [];
    for (let i = 0; i < paths.length; i++) {
      if (paths.get(i)!.toLowerCase().includes(q)) {
        result.push({ path: paths.get(i)!, index: i });
      }
    }
    return result;
  }, [paths, searchQuery]);

  // 路径验证状态
  const validations = useMemo(() => {
    const seen = new Set<string>();
    return filtered.map(({ path }) => {
      const lower = path.toLowerCase();
      const isDuplicate = seen.has(lower);
      seen.add(lower);
      return {
        isValid: validatePath(path),
        isDuplicate,
      };
    });
  }, [filtered]);

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
      // 双击编辑 — 由 AppShell 处理（通过事件）
      window.dispatchEvent(
        new CustomEvent('path-dblclick', {
          detail: { index: realIndex, path: paths.get(realIndex) },
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
            style={{
              backgroundColor: 'var(--app-list-alt)',
              color: 'var(--app-fg)',
            }}
          >
            <th className="w-8 px-2 py-1">#</th>
            <th className="px-2 py-1">路径</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map(({ path, index }, rowIdx) => {
            const v = validations[rowIdx];
            const isSelected = selectedIndices.includes(index);
            let textColor = 'var(--app-fg)';
            if (!v.isValid) textColor = '#dc3545';
            else if (v.isDuplicate) textColor = '#fd7e14';

            return (
              <tr
                key={index}
                onClick={(e) => handleClick(index, e)}
                onDoubleClick={() => handleDoubleClick(index)}
                className="cursor-pointer select-none"
                style={{
                  backgroundColor: isSelected
                    ? 'rgba(59, 130, 246, 0.3)'
                    : rowIdx % 2 === 0
                      ? 'var(--app-list-bg)'
                      : 'var(--app-list-alt)',
                }}
              >
                <td
                  className="px-2 py-0.5 text-xs opacity-50"
                  style={{ color: 'var(--app-fg)' }}
                >
                  {index + 1}
                </td>
                <td className="px-2 py-0.5 text-sm" style={{ color: textColor }}>
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
