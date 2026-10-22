import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * `historyRevealEnvValue` 的形状校验。
 *
 * 这条命令是**唯一**返回历史里环境变量明文的出口，所以它的契约必须守住：
 * - 必须返回字符串；
 * - 后端返回非字符串时**必须抛错**，不能静默转成 `String(x)` ——
 *   那会把 `undefined` 变成字符串 `"undefined"` 显示在 UI 上。
 *
 * 与 `reveal_env_var` 的区别：后者返回 `{ value, revision }`，后者是「当前注册表」；
 * 前者返回裸字符串，因为历史快照的 revision 是采集时点的，对调用方没有意义。
 */

// `backend.ts` 在模块顶层引用 `@tauri-apps/api/core` 的 invoke，
// 这里把它替换成一个可由每个用例改写的桩。
const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe('historyRevealEnvValue', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('返回后端给出的明文字符串', async () => {
    invokeMock.mockResolvedValue('super-secret');
    const { backend } = await import('@/services/backend');
    await expect(backend.historyRevealEnvValue('snap-1', 'user', 'MY_TOKEN')).resolves.toBe(
      'super-secret',
    );
    expect(invokeMock).toHaveBeenCalledWith('history_reveal_env_value', {
      uid: 'snap-1',
      hive: 'user',
      name: 'MY_TOKEN',
    });
  });

  it('后端返回非字符串时抛错，不得静默转成字符串', async () => {
    invokeMock.mockResolvedValue(undefined);
    const { backend } = await import('@/services/backend');
    await expect(backend.historyRevealEnvValue('snap-1', 'user', 'MY_TOKEN')).rejects.toThrow(
      /必须返回字符串/,
    );
  });

  it('后端返回 null 时同样抛错（null 不是「空值」的合法表示）', async () => {
    invokeMock.mockResolvedValue(null);
    const { backend } = await import('@/services/backend');
    await expect(backend.historyRevealEnvValue('snap-1', 'user', 'X')).rejects.toThrow(
      /必须返回字符串/,
    );
  });

  it('允许空字符串（变量存在但值为空）', async () => {
    invokeMock.mockResolvedValue('');
    const { backend } = await import('@/services/backend');
    await expect(backend.historyRevealEnvValue('snap-1', 'user', 'EMPTY')).resolves.toBe('');
  });
});
