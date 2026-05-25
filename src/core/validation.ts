/**
 * 路径格式验证 — 对应 C 版 import_export.c:is_valid_path_format()
 */

/** 检查路径是否符合 Windows 路径格式 */
export function is_valid_path_format(path: string): boolean {
  if (!path || path.trim() === '') return false;

  // UNC 路径: \\server\share
  if (path.startsWith('\\\\') || path.startsWith('//')) return true;

  // 驱动器字母: C:\... 或 C:/
  if (/^[a-zA-Z]:[/\\]/.test(path)) return true;

  // 环境变量: %VAR%
  if (path.includes('%')) return true;

  // 包含路径分隔符的相对路径
  if (path.includes('/') || path.includes('\\')) return true;

  return false;
}

/** 连接 PATH 字符串（用分号） */
export function join_path(paths: string[]): string {
  return paths.join(';');
}

/** 分割 PATH 字符串 */
export function split_path(raw: string): string[] {
  return raw
    .split(';')
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}
