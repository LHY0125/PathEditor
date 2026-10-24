import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * 搜索 store 的「空查询不发请求」契约。
 *
 * 这条契约有两层：core 的 `search()` 对空查询返回空数组，
 * 前端 store 的 `run()` 直接跳过 IPC。两层各守一次——
 * 只有 core 那层的话，每次输入框删空都会白跑一次跨进程调用。
 */

const searchMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@/services/backend', () => ({
  backend: {
    search: (...args: unknown[]) => searchMock(...args),
  },
}));

describe('search store', () => {
  beforeEach(async () => {
    searchMock.mockReset();
    const { useSearchStore } = await import('@/store/search-store');
    useSearchStore.getState().reset();
  });

  it('空查询不发请求，并清空结果', async () => {
    const { useSearchStore } = await import('@/store/search-store');
    const s = useSearchStore.getState();
    s.setQuery('   ');
    await useSearchStore.getState().run();

    expect(searchMock).not.toHaveBeenCalled();
    expect(useSearchStore.getState().hits).toEqual([]);
    expect(useSearchStore.getState().submitted).toBe('');
  });

  it('非空查询发一次请求并写入结果', async () => {
    searchMock.mockResolvedValue([
      { kind: 'path', refUid: 'snap#system', snippet: '[jdk21]', rank: -1 },
    ]);
    const { useSearchStore } = await import('@/store/search-store');
    useSearchStore.getState().setQuery('jdk21');
    await useSearchStore.getState().run();

    expect(searchMock).toHaveBeenCalledWith('jdk21', 50);
    expect(useSearchStore.getState().hits).toHaveLength(1);
    expect(useSearchStore.getState().submitted).toBe('jdk21');
  });

  it('查询串首尾空白被 trim 后再提交', async () => {
    searchMock.mockResolvedValue([]);
    const { useSearchStore } = await import('@/store/search-store');
    useSearchStore.getState().setQuery('  jdk  ');
    await useSearchStore.getState().run();

    expect(searchMock).toHaveBeenCalledWith('jdk', 50);
  });

  it('降级模式（code=database）给出可读文案', async () => {
    searchMock.mockRejectedValue({ code: 'database', message: '历史库不可用' });
    const { useSearchStore } = await import('@/store/search-store');
    useSearchStore.getState().setQuery('x');
    await useSearchStore.getState().run();

    // 文案已走 i18n：测试环境语言可能是 en，断言两种语言都接受
    expect(useSearchStore.getState().error).toMatch(/降级|degraded/i);
  });

  it('请求期间查询被改动时，丢弃过期结果（竞态守卫）', async () => {
    let resolveFn: (v: unknown) => void = () => {};
    searchMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveFn = resolve;
        }),
    );
    const { useSearchStore } = await import('@/store/search-store');
    useSearchStore.getState().setQuery('first');
    const pending = useSearchStore.getState().run();

    // 请求未返回时改查询
    useSearchStore.getState().setQuery('second');
    resolveFn([{ kind: 'path', refUid: 'r', snippet: 's', rank: 0 }]);
    await pending;

    // 旧结果不得写入（否则用户看到的是上一次查询的结果）
    expect(useSearchStore.getState().hits).toEqual([]);
  });
});
