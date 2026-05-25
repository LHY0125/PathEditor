/**
 * 路径管理器 — 对应 C 版 path_manager.c
 * 提供路径增删移清理等 CRUD 操作的纯逻辑
 */
import { StringList } from './string-list';

/** 删除指定索引的路径 */
export function pathRemoveAt(list: StringList, index: number): void {
  list.removeAt(index);
}

/** 上移路径（调整优先级） */
export function pathMoveUp(list: StringList, index: number): boolean {
  if (index <= 0 || index >= list.length) return false;
  list.swap(index, index - 1);
  return true;
}

/** 下移路径（调整优先级） */
export function pathMoveDown(list: StringList, index: number): boolean {
  if (index < 0 || index >= list.length - 1) return false;
  list.swap(index, index + 1);
  return true;
}

/** 标记路径的有效性（调用方负责提供验证函数和展开 env vars） */
export interface PathValidation {
  isValid: boolean;
  isDuplicate: boolean;
  isEnvVar: boolean;
}

/**
 * 分析路径列表中各条目的状态
 * validateFn: 验证路径是否有效（需调用 Rust validate_path）
 */
export function analyzePaths(
  list: StringList,
  validateFn: (path: string) => boolean,
): PathValidation[] {
  const result: PathValidation[] = [];
  const seen = new Set<string>();

  for (let i = 0; i < list.length; i++) {
    const path = list.get(i)!;
    const lower = path.toLowerCase();
    const isDuplicate = seen.has(lower);
    seen.add(lower);

    result.push({
      isValid: validateFn(path),
      isDuplicate,
      isEnvVar: path.includes('%'),
    });
  }

  return result;
}

/**
 * 批量删除选中的索引（从大到小排序以避免偏移）
 */
export function batchRemoveAt(list: StringList, indices: number[]): void {
  const sorted = [...indices].sort((a, b) => b - a);
  for (const idx of sorted) {
    list.removeAt(idx);
  }
}

/**
 * 一键清理 — 移除无效路径和重复路径
 * 从后往前操作以避免索引偏移
 * 返回被移除的路径数量
 */
export function pathClean(
  list: StringList,
  validateFn: (path: string) => boolean,
): string[] {
  const analysis = analyzePaths(list, validateFn);
  const removed: string[] = [];

  for (let i = analysis.length - 1; i >= 0; i--) {
    const a = analysis[i];
    // 移除无效或重复的路径
    if (!a.isValid || a.isDuplicate) {
      removed.unshift(list.get(i)!);
      list.removeAt(i);
    }
  }

  return removed;
}
