/**
 * 是否是结构化 `CoreError`（`parseCoreError` 抛出的是普通对象，不是 `Error` 实例）。
 *
 * **唯一实现**：7 个 store/组件各自复制过一份形状判断，任一处改动都会让
 * `String(err)` 退化成 `"[object Object]"` 的旧 bug 复发。
 */
export function isCoreError(error: unknown): error is { code: string; message: string } {
  if (typeof error !== 'object' || error === null) return false;
  if (!('code' in error) || typeof error.code !== 'string') return false;
  if (!('message' in error) || typeof error.message !== 'string') return false;
  return true;
}

/**
 * 从 IPC rejection 中取用户可读文案。
 *
 * `parseCoreError` 抛出的是普通对象 `{code, message}`，不是 `Error` 实例；
 * 直接 `String(err)` 会得到 `[object Object]`。所有展示层错误文案都走这里，
 * 避免每个调用点各写一份形状判断。
 */
export function errorMessage(error: unknown, fallbackByCode?: (code: string) => string): string {
  if (isCoreError(error)) {
    if (error.message.trim().length > 0) return error.message;
    return fallbackByCode?.(error.code) ?? String(error);
  }
  return error instanceof Error ? error.message : String(error);
}
