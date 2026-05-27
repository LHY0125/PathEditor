/**
 * 路径管理器 — 不可变的 PathEntry[] 操作
 */

import type { PathEntry } from './path-entry';

export interface PathValidation {
  isValid: boolean;
  isDuplicate: boolean;
  isEnvVar: boolean;
}

export function analyzePaths(
  paths: readonly PathEntry[],
  validateFn: (path: string) => boolean,
): PathValidation[] {
  const result: PathValidation[] = [];
  const seen = new Set<string>();

  for (const entry of paths) {
    const lower = entry.path.toLowerCase();
    const isDuplicate = seen.has(lower);
    seen.add(lower);
    result.push({ isValid: validateFn(entry.path), isDuplicate, isEnvVar: entry.path.includes('%') });
  }

  return result;
}

/** 从数组中移除无效和重复路径，返回 [新数组, 被移除的路径] */
export function pathClean(
  paths: readonly PathEntry[],
  validateFn: (path: string) => boolean,
): [PathEntry[], PathEntry[]] {
  const analysis = analyzePaths(paths, validateFn);
  const kept: PathEntry[] = [];
  const removed: PathEntry[] = [];

  for (let i = 0; i < paths.length; i++) {
    const a = analysis[i];
    if (!a.isValid || a.isDuplicate) {
      removed.push(paths[i]);
    } else {
      kept.push(paths[i]);
    }
  }

  return [kept, removed];
}
