/**
 * 路径管理器 — 不可变的 string[] 操作
 */

export interface PathValidation {
  isValid: boolean;
  isDuplicate: boolean;
  isEnvVar: boolean;
}

export function analyzePaths(
  paths: readonly string[],
  validateFn: (path: string) => boolean,
): PathValidation[] {
  const result: PathValidation[] = [];
  const seen = new Set<string>();

  for (const path of paths) {
    const lower = path.toLowerCase();
    const isDuplicate = seen.has(lower);
    seen.add(lower);
    result.push({ isValid: validateFn(path), isDuplicate, isEnvVar: path.includes('%') });
  }

  return result;
}

/** 从数组中移除无效和重复路径，返回 [新数组, 被移除的路径] */
export function pathClean(
  paths: readonly string[],
  validateFn: (path: string) => boolean,
): [string[], string[]] {
  const analysis = analyzePaths(paths, validateFn);
  const kept: string[] = [];
  const removed: string[] = [];

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
