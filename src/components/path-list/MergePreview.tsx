import { useMemo } from 'react';
import { useAppStore } from '@/store/app-store';

export function MergePreview() {
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const searchQuery = useAppStore((s) => s.searchQuery);

  const allPaths = useMemo(() => {
    const result: { path: string; source: '系统' | '用户'; index: number }[] = [];
    sysPaths.all.forEach((p, i) => result.push({ path: p, source: '系统' as const, index: i }));
    userPaths.all.forEach((p, i) => result.push({ path: p, source: '用户' as const, index: i }));

    if (!searchQuery) return result;
    const q = searchQuery.toLowerCase();
    return result.filter((r) => r.path.toLowerCase().includes(q));
  }, [sysPaths, userPaths, searchQuery]);

  return (
    <div className="flex-1 overflow-auto">
      <table className="w-full border-collapse">
        <thead>
          <tr
            className="sticky top-0 z-10 text-left text-xs uppercase"
            style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
          >
            <th className="w-10 px-2 py-1">#</th>
            <th className="px-2 py-1">路径</th>
            <th className="w-16 px-2 py-1">来源</th>
          </tr>
        </thead>
        <tbody>
          {allPaths.map(({ path, source, index }, rowIdx) => (
            <tr
              key={`${source}-${index}`}
              style={{
                backgroundColor:
                  rowIdx % 2 === 0 ? 'var(--app-list-bg)' : 'var(--app-list-alt)',
                color: 'var(--app-fg)',
              }}
            >
              <td className="px-2 py-0.5 text-xs opacity-50">{rowIdx + 1}</td>
              <td className="px-2 py-0.5 text-sm">{path}</td>
              <td className="px-2 py-0.5 text-xs opacity-60">{source}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
