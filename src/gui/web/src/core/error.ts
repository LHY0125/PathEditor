/**
 * 从 IPC rejection 中取用户可读文案。
 *
 * `parseCoreError` 抛出的是普通对象 `{code, message}`，不是 `Error` 实例；
 * 直接 `String(err)` 会得到 `[object Object]`。所有展示层错误文案都走这里，
 * 避免每个调用点各写一份形状判断。
 */
export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === 'string' && message.trim().length > 0) return message;
  }
  return error instanceof Error ? error.message : String(error);
}
