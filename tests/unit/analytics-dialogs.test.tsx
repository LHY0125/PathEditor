import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import i18n from '@/i18n';

/**
 * 对比与引用图组件的守卫。
 *
 * 这三条断言对应本轮修掉/新增的三个维度：
 * 1. **双 hive**：只显示 system 会让「只有用户 PATH 变了」看起来像没差异。
 * 2. **toggled**（超出计划的维度）：同一条路径从启用变禁用是真实变化。
 * 3. **env 显式文案**：不能让用户以为「环境变量没变化」。
 */

// 测试环境的 navigator 是 en；断言中文文案前必须显式切语言。
// 用 `beforeAll` 而不是每个用例里切：切换是全局副作用，重复切换会与其他
// 测试文件的 i18n 状态互相干扰。
beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效：
// 不显式清理会让上一个用例的 DOM 残留，`getByText` 报「Found multiple elements」。
afterEach(() => {
  cleanup();
});

const compareMock = vi.fn();
const refsDependentsMock = vi.fn();
const refsDependenciesMock = vi.fn();
const refsCyclesMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@/services/backend', () => ({
  backend: {
    compareSnapshots: (...a: unknown[]) => compareMock(...a),
    refsDependents: (...a: unknown[]) => refsDependentsMock(...a),
    refsDependencies: (...a: unknown[]) => refsDependenciesMock(...a),
    refsCycles: (...a: unknown[]) => refsCyclesMock(...a),
  },
}));

const emptyDiff = { added: [], removed: [], reordered: [], toggled: [] };

describe('CompareDialog', () => {
  beforeEach(() => {
    compareMock.mockReset();
  });

  it('同时显示 system 与 user 两侧', async () => {
    compareMock.mockResolvedValue({
      system: { ...emptyDiff, added: [{ path: 'C:\\SYS\\NEW', enabled: true }] },
      user: { ...emptyDiff, added: [{ path: 'C:\\USR\\NEW', enabled: true }] },
      envAdded: [],
      envRemoved: [],
      envModified: [],
    });
    const { CompareDialog } = await import('@/components/history/CompareDialog');
    render(<CompareDialog leftUid="a" rightUid="b" onClose={() => {}} />);

    await waitFor(() => expect(screen.getByText(/C:\\SYS\\NEW/)).toBeTruthy());
    // user 侧的变化也必须出现——只比 system 时这里会是「无变化」
    expect(screen.getByText(/C:\\USR\\NEW/)).toBeTruthy();
  });

  it('启用状态变化也显示（toggled）', async () => {
    compareMock.mockResolvedValue({
      system: {
        ...emptyDiff,
        toggled: [{ path: 'C:\\SAME', leftEnabled: true, rightEnabled: false }],
      },
      user: emptyDiff,
      envAdded: [],
      envRemoved: [],
      envModified: [],
    });
    const { CompareDialog } = await import('@/components/history/CompareDialog');
    render(<CompareDialog leftUid="a" rightUid="b" onClose={() => {}} />);

    await waitFor(() => expect(screen.getByText(/C:\\SAME/)).toBeTruthy());
    // 核心断言：system 侧因为 toggled 非空，**不得**显示「无变化」；
    // user 侧全空，应显示一次。若实现漏了 toggled，这里会得到两次「无变化」。
    expect(screen.queryAllByText(/无变化/)).toHaveLength(1);
  });

  it('env 对比显式说明本版本不支持', async () => {
    compareMock.mockResolvedValue({
      system: emptyDiff,
      user: emptyDiff,
      envAdded: [],
      envRemoved: [],
      envModified: [],
    });
    const { CompareDialog } = await import('@/components/history/CompareDialog');
    render(<CompareDialog leftUid="a" rightUid="b" onClose={() => {}} />);

    await waitFor(() => expect(screen.getByText(/本版本不支持环境变量对比/)).toBeTruthy());
  });
});

describe('RefGraphDialog', () => {
  beforeEach(() => {
    refsDependentsMock.mockReset();
    refsDependenciesMock.mockReset();
    refsCyclesMock.mockReset();
  });

  it('同时显示反向、前向与环三个方向', async () => {
    refsDependentsMock.mockResolvedValue([
      { fromHive: 'system', fromName: 'PATH', toName: 'JAVA_HOME', depth: 1 },
    ]);
    refsDependenciesMock.mockResolvedValue([
      { fromHive: 'system', fromName: 'JAVA_HOME', toName: 'JDK_HOME', depth: 1 },
    ]);
    refsCyclesMock.mockResolvedValue([['JAVA_HOME', 'JDK_HOME']]);

    const { RefGraphDialog } = await import('@/components/env/RefGraphDialog');
    render(<RefGraphDialog name="JAVA_HOME" onClose={() => {}} />);

    // 反向：谁引用了它
    await waitFor(() => expect(screen.getByText('system/PATH')).toBeTruthy());
    // 前向：它依赖了谁
    expect(screen.getByText('JDK_HOME')).toBeTruthy();
    // 环：该变量参与的环要高亮显示
    expect(screen.getByText(/JAVA_HOME → JDK_HOME → JAVA_HOME/)).toBeTruthy();
  });

  it('降级模式给出可读文案', async () => {
    refsDependentsMock.mockRejectedValue({ code: 'database', message: '历史库不可用' });
    refsDependenciesMock.mockResolvedValue([]);
    refsCyclesMock.mockResolvedValue([]);

    const { RefGraphDialog } = await import('@/components/env/RefGraphDialog');
    render(<RefGraphDialog name="X" onClose={() => {}} />);

    await waitFor(() => expect(screen.getByText(/降级模式/)).toBeTruthy());
  });
});
