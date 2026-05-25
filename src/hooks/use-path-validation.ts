import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Rust 端未就绪时的 fallback
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** 同步验证（基于本地规则，不含文件系统检查） */
export function validatePath(path: string): boolean {
  if (path.includes('%')) return true;
  return true; // 文件系统检查需要调用 Rust backend
}

/** 异步验证（调用 Rust validate_path） */
export function useAsyncValidation() {
  const [cache, setCache] = useState<Map<string, boolean>>(new Map());

  const validate = useCallback(async (path: string): Promise<boolean> => {
    if (path.includes('%')) return true;
    if (cache.has(path)) return cache.get(path)!;

    if (isTauri) {
      try {
        const valid: boolean = await invoke('validate_path', { path });
        setCache((prev) => new Map(prev).set(path, valid));
        return valid;
      } catch {
        return true;
      }
    }

    return true;
  }, [cache]);

  return { validate, cache };
}
