import { useEffect, useMemo, useRef, useState } from 'react';
import { useAppStore } from '@/store/app-store';
import { backend, type ConflictEntry, type ToolGroup } from '@/services/backend';

function getEnabledPaths(): string[] {
  const { sysPaths, userPaths } = useAppStore.getState();
  return [
    ...sysPaths.filter((entry) => entry.enabled),
    ...userPaths.filter((entry) => entry.enabled),
  ].map((entry) => entry.path);
}

export function useAnalyzeData(open: boolean) {
  const [loading, setLoading] = useState(false);
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  const [toolGroups, setToolGroups] = useState<ToolGroup[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState('');
  const prevOpen = useRef(false);

  useEffect(() => {
    if (!open) {
      prevOpen.current = false;
      return;
    }
    if (prevOpen.current) return;
    prevOpen.current = true;
    setLoading(true);
    setError('');
    backend
      .scanPaths(getEnabledPaths())
      .then((result) => {
        setConflicts(result.conflicts);
        setToolGroups(result.tools);
      })
      .catch((scanError) => setError(String(scanError)))
      .finally(() => setLoading(false));
  }, [open]);

  const filteredTools = useMemo(() => {
    if (!searchQuery.trim()) return toolGroups;
    const query = searchQuery.toLowerCase();
    return toolGroups
      .map((group) => ({
        ...group,
        exes: group.exes.filter((exe) => exe.toLowerCase().includes(query)),
      }))
      .filter((group) => group.exes.length > 0);
  }, [toolGroups, searchQuery]);

  return {
    loading,
    error,
    conflicts,
    filteredTools,
    searchQuery,
    setSearchQuery,
  };
}
