import { useState, useEffect, useMemo, useCallback } from 'react';
import { useAppStore } from '@/store/app-store';
import { invoke } from '@tauri-apps/api/core';

interface PathTableProps {
  tabId: 'system' | 'user';
}

interface PathRow {
  path: string;
  index: number;
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

  // 本次会话中已验证过的路径缓存（key=path, value=isValid）
  const [validationCache, setValidationCache] = useState<Map<string, boolean>>(new Map());
  // 环境变量展开结果缓存（key=path, value=expanded）
  const [expandedCache, setExpandedCache] = useState<Map<string, string>>(new Map());

  // 过滤搜索
  const filtered = useMemo<PathRow[]>(() => {
    if (!searchQuery) return paths.all.map((p, i) => ({ path: p, index: i }));
    const q = searchQuery.toLowerCase();
    const result: PathRow[] = [];
    for (let i = 0; i < paths.length; i++) {
      const p = paths.get(i)!;
      if (p.toLowerCase().includes(q)) result.push({ path: p, index: i });
    }
    return result;
  }, [paths, searchQuery]);

  // 异步验证未缓存的路径
  useEffect(() => {
    let cancelled = false;
    const allPaths = paths.all;

    // 找出未缓存的路径
    const toValidate = allPaths.filter((p) => !validationCache.has(p));
    if (toValidate.length === 0) return;

    // 批量验证（限制并发 20）
    const batch = toValidate.slice(0, 20);
    Promise.all(
      batch.map(async (p): Promise<[string, boolean]> => {
        try {
          if (p.includes('%')) return [p, true];
          const valid: boolean = await invoke('validate_path', { path: p });
          return [p, valid];
        } catch {
          return [p, true];
        }
      }),
    ).then((results) => {
      if (cancelled) return;
      setValidationCache((prev) => {
        const next = new Map(prev);
        for (const [p, v] of results) {
          next.set(p, v);
        }
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [paths, validationCache]);

  // 异步展开环境变量（用于 tooltip）
  useEffect(() => {
    let cancelled = false;
    const toExpand = paths.all.filter(
      (p) => p.includes('%') && !expandedCache.has(p),
    );
    if (toExpand.length === 0) return;

    Promise.all(
      toExpand.map(async (p): Promise<[string, string]> => {
        try {
          const expanded: string = await invoke('expand_env_vars', { path: p });
          return [p, expanded !== p ? expanded : ''];
        } catch {
          return [p, ''];
        }
      }),
    ).then((results) => {
      if (cancelled) return;
      setExpandedCache((prev) => {
        const next = new Map(prev);
        for (const [p, v] of results) {
          next.set(p, v);
        }
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [paths, expandedCache]);

  // 所有路径都默认有效（异步验证结果回来后再精确染色）
  const validations = useMemo(() => {
    const seen = new Set<string>();
    return filtered.map(({ path }) => {
      const lower = path.toLowerCase();
      const isDuplicate = seen.has(lower);
      seen.add(lower);
      return {
        isValid: validationCache.get(path) ?? true,
        isDuplicate,
        isEnvVar: path.includes('%'),
      };
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
            style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
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
                <td className="w-8 px-2 py-0.5 text-xs opacity-50" style={{ color: 'var(--app-fg)' }}>
                  {index + 1}
                </td>
                <td
                  className="px-2 py-0.5 text-sm truncate max-w-2xl"
                  style={{ color: textColor }}
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
