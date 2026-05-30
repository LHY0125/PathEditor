import { useState, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { PathEntry } from '@/core/path-entry';

export type ValidationState = 'valid' | 'invalid' | 'unknown';

/**
 * 异步验证路径目录是否真实存在 + 展开环境变量
 * 缓存结果避免重复 IPC 调用。
 * setState 仅在异步 .then() 回调中调用（符合 React 规则），
 * 不存在路径的缓存清理通过 useMemo 派生。
 */
export function usePathValidation(paths: readonly PathEntry[]) {
  const validatedRef = useRef<Set<string>>(new Set());
  const expandedRef = useRef<Set<string>>(new Set());
  const [validationCache, setValidationCache] = useState<Map<string, ValidationState>>(new Map());
  const [expandedCache, setExpandedCache] = useState<Map<string, string>>(new Map());

  // 仅保留当前 paths 中存在的条目（派生 state，不在 effect 中同步 setState）
  const currentKeys = useMemo(() => new Set(paths.map((p) => p.path)), [paths]);
  const cleanedValidationCache = useMemo(() => {
    const next = new Map(validationCache);
    let changed = false;
    for (const key of next.keys()) {
      if (!currentKeys.has(key)) {
        next.delete(key);
        changed = true;
      }
    }
    return changed ? next : validationCache;
  }, [validationCache, currentKeys]);

  const cleanedExpandedCache = useMemo(() => {
    const next = new Map(expandedCache);
    let changed = false;
    for (const key of next.keys()) {
      if (!currentKeys.has(key)) {
        next.delete(key);
        changed = true;
      }
    }
    return changed ? next : expandedCache;
  }, [expandedCache, currentKeys]);

  // 同步清理 ref（ref 不能在 render 期间修改，放在 effect 中不 setState 是安全的）
  useEffect(() => {
    for (const key of validatedRef.current) {
      if (!currentKeys.has(key)) validatedRef.current.delete(key);
    }
    for (const key of expandedRef.current) {
      if (!currentKeys.has(key)) expandedRef.current.delete(key);
    }
  }, [currentKeys]);

  // 异步验证路径（setState 在 .then() 回调中，符合 React 规则）
  useEffect(() => {
    let cancelled = false;
    const toValidate = paths.filter((p) => !validatedRef.current.has(p.path));
    if (toValidate.length === 0) return;

    const batch = toValidate.slice(0, 20);
    Promise.all(
      batch.map(
        async (p): Promise<[string, ValidationState]> => {
          try {
            if (p.path.includes('%')) return [p.path, 'valid'];
            const valid: boolean = await invoke('validate_path', { path: p.path });
            return [p.path, valid ? 'valid' : 'invalid'];
          } catch {
            return [p.path, 'unknown'];
          }
        },
      ),
    ).then((results) => {
      if (cancelled) return;
      for (const [p] of results) validatedRef.current.add(p);
      setValidationCache((prev) => {
        const next = new Map(prev);
        for (const [p, v] of results) next.set(p, v);
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [paths]);

  // 异步展开环境变量（setState 在 .then() 回调中）
  useEffect(() => {
    let cancelled = false;
    const toExpand = paths.filter(
      (p) => p.path.includes('%') && !expandedRef.current.has(p.path),
    );
    if (toExpand.length === 0) return;

    const batch = toExpand.slice(0, 20);
    Promise.all(
      batch.map(
        async (p): Promise<[string, string]> => {
          try {
            const expanded: string = await invoke('expand_env_vars', { path: p.path });
            return [p.path, expanded !== p.path ? expanded : ''];
          } catch {
            return [p.path, ''];
          }
        },
      ),
    ).then((results) => {
      if (cancelled) return;
      for (const [p] of results) expandedRef.current.add(p);
      setExpandedCache((prev) => {
        const next = new Map(prev);
        for (const [p, v] of results) next.set(p, v);
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [paths]);

  return { validationCache: cleanedValidationCache, expandedCache: cleanedExpandedCache };
}
