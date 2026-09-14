import { useEffect, useMemo, useRef, useState } from 'react';
import type { PathEntry } from '@/core/path-entry';
import { backend } from '@/services/backend';

export type ValidationState = 'pending' | 'valid' | 'invalid' | 'unknown';

/** 有界并发，避免 1000 条 PATH 同时发起 IPC。 */
export const VALIDATION_CONCURRENCY = 8;

type ValidationCache = Map<string, ValidationState>;
type ExpandedCache = Map<string, string>;

interface InspectResult {
  path: string;
  state: ValidationState;
  expanded: string;
}

async function inspectPath(path: string): Promise<InspectResult> {
  try {
    if (!path.includes('%')) {
      const valid = await backend.validatePath(path);
      return { path, state: valid ? 'valid' : 'invalid', expanded: '' };
    }

    const expanded = await backend.expandEnvVars(path);
    if (!expanded || expanded === path || expanded.includes('%')) {
      return { path, state: 'unknown', expanded: expanded === path ? '' : expanded };
    }

    const valid = await backend.validatePath(expanded);
    return { path, state: valid ? 'valid' : 'invalid', expanded };
  } catch {
    return { path, state: 'unknown', expanded: '' };
  }
}

/**
 * 异步验证路径目录是否真实存在，并展开环境变量。
 *
 * 同一路径共享 in-flight Promise：列表 rerender 时，新 effect 会等待正在进行的请求并
 * 应用结果，不会因为取消旧批次而永久停在 pending。
 */
export function usePathValidation(paths: readonly PathEntry[]) {
  const validationRef = useRef<ValidationCache>(new Map());
  const expandedRef = useRef<ExpandedCache>(new Map());
  const inFlightRef = useRef<Map<string, Promise<InspectResult>>>(new Map());
  const [validationCache, setValidationCache] = useState<ValidationCache>(new Map());
  const [expandedCache, setExpandedCache] = useState<ExpandedCache>(new Map());

  const currentKeys = useMemo(() => new Set(paths.map((entry) => entry.path)), [paths]);

  const cleanedValidationCache = useMemo(() => {
    const next = new Map(validationCache);
    for (const key of next.keys()) {
      if (!currentKeys.has(key)) next.delete(key);
    }
    return next;
  }, [validationCache, currentKeys]);

  const cleanedExpandedCache = useMemo(() => {
    const next = new Map(expandedCache);
    for (const key of next.keys()) {
      if (!currentKeys.has(key)) next.delete(key);
    }
    return next;
  }, [expandedCache, currentKeys]);

  useEffect(() => {
    for (const key of validationRef.current.keys()) {
      if (!currentKeys.has(key)) validationRef.current.delete(key);
    }
    for (const key of expandedRef.current.keys()) {
      if (!currentKeys.has(key)) expandedRef.current.delete(key);
    }
  }, [currentKeys]);

  useEffect(() => {
    let cancelled = false;
    const uniquePaths = [...new Set(paths.map((entry) => entry.path))];

    const initialPending: ValidationCache = new Map(validationRef.current);
    for (const path of uniquePaths) {
      if (!initialPending.has(path)) initialPending.set(path, 'pending');
    }
    validationRef.current = initialPending;

    const getInFlight = (path: string): Promise<InspectResult> => {
      const existing = inFlightRef.current.get(path);
      if (existing) return existing;

      const promise = inspectPath(path).finally(() => {
        if (inFlightRef.current.get(path) === promise) {
          inFlightRef.current.delete(path);
        }
      });
      inFlightRef.current.set(path, promise);
      return promise;
    };

    const run = async () => {
      for (let index = 0; index < uniquePaths.length; index += VALIDATION_CONCURRENCY) {
        if (cancelled) return;

        const batch = uniquePaths
          .slice(index, index + VALIDATION_CONCURRENCY)
          .filter((path) => validationRef.current.get(path) === 'pending');
        if (batch.length === 0) continue;

        const results = await Promise.all(batch.map((path) => getInFlight(path)));
        if (cancelled) return;

        const nextValidation = new Map(validationRef.current);
        const nextExpanded = new Map(expandedRef.current);
        for (const result of results) {
          nextValidation.set(result.path, result.state);
          if (result.expanded) nextExpanded.set(result.path, result.expanded);
        }
        validationRef.current = nextValidation;
        expandedRef.current = nextExpanded;
        setValidationCache(nextValidation);
        setExpandedCache(nextExpanded);
      }
    };

    void run();
    return () => {
      cancelled = true;
    };
  }, [paths]);

  return { validationCache: cleanedValidationCache, expandedCache: cleanedExpandedCache };
}
