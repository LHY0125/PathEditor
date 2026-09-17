import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// 异步工厂 + vi.mocked(backend)：与 env-var-table.test.tsx / env-store.test.ts 一致。
// 不能在工厂外引用 mock 变量 —— vi.mock 会被提升到 import 之前，外层 const 仍处于 TDZ。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  return {
    backend: {
      listAllEnvVars: viModule.fn(),
      loadPathSnapshot: viModule.fn(),
      getPathCapabilities: viModule.fn(),
      revealEnvVar: viModule.fn(),
      updateEnvVar: viModule.fn(),
      createEnvVar: viModule.fn(),
      deleteEnvVar: viModule.fn(),
      expandEnvVars: viModule.fn(),
      validatePath: viModule.fn(),
    },
  };
});

// i18n 用**部分 mock**：保留 initReactI18next 等真实导出（src/i18n 在模块加载时要用），
// 只覆盖 useTranslation，并以真实 zh-CN 词条取值，断言基于可见文案而非 i18n key。
vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  const zh = ((await import('@/i18n/locales/zh-CN.json')).default ?? {}) as Record<string, unknown>;
  const t = (key: string): string => {
    let node: unknown = zh;
    for (const part of key.split('.')) {
      if (node === null || typeof node !== 'object') return key;
      node = (node as Record<string, unknown>)[part];
    }
    return typeof node === 'string' ? node : key;
  };
  return { ...actual, useTranslation: () => ({ t }) };
});

// jsdom 下虚拟滚动容器高度为 0，真实实现不会渲染任何行；按 merge-preview.test.tsx
// 的既有做法 mock 掉，让 getVirtualItems 返回全部条目。
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: (options: Record<string, number>) => ({
    getVirtualItems: () =>
      Array.from({ length: options.count }).map((_, index) => ({
        index,
        start: index * 34,
        size: 34,
        key: `mock-key-${index}`,
      })),
    getTotalSize: () => options.count * 34,
    measureElement: () => {},
  }),
}));

import { AppShell } from '@/components/layout/AppShell';
import { backend } from '@/services/backend';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

const mockBackend = vi.mocked(backend);

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

/** 取真实的 drop 容器（PATH Tab 的拖放区）。 */
function dropZone(container: HTMLElement): Element {
  const zone = container.querySelector('[data-testid="path-drop-zone"]');
  if (zone === null) throw new Error('未找到拖放区');
  return zone;
}

/** 构造一次“拖入一个文件夹”的 drop 事件。 */
function dropFolder(zone: Element, path: string): void {
  fireEvent.drop(zone, {
    dataTransfer: {
      items: [{ webkitGetAsEntry: () => ({ isDirectory: true }) }],
      files: [{ path }],
    },
  });
}

beforeEach(() => {
  // resetAllMocks 而非 clearAllMocks：后者保留 mock 实现，会导致跨用例残留。
  vi.resetAllMocks();
  mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [] });
  mockBackend.loadPathSnapshot.mockResolvedValue({ system: [], user: [] });
  mockBackend.getPathCapabilities.mockResolvedValue({
    canReadSystem: true,
    canWriteSystem: true,
    canReadUser: true,
    canWriteUser: true,
  });
  mockBackend.expandEnvVars.mockResolvedValue('');
  mockBackend.validatePath.mockResolvedValue(true);

  useAppStore.setState({
    activeTab: 'system',
    isModified: false,
    sysPaths: [],
    userPaths: [],
    isAdmin: false,
    pathCapabilities: {
      canReadSystem: false,
      canWriteSystem: false,
      canReadUser: false,
      canWriteUser: false,
    },
  });
  useEnvStore.setState({ draft: new Map(), snapshot: { system: [], user: [] } });
});

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效：不显式清理会让
// screen（绑定 document.body）跨用例累积多个 AppShell，getByText 直接报“匹配到多个元素”。
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('AppShell Tab 结构', () => {
  it('渲染 4 个 Tab 且「全部变量」可用', () => {
    render(<AppShell />);
    expect(screen.getByText('系统 PATH')).not.toBeNull();
    expect(screen.getByText('用户 PATH')).not.toBeNull();
    expect(screen.getByText('全部变量')).not.toBeNull();
    expect(screen.getByText('合并预览')).not.toBeNull();
  });

  it('切到「全部变量」后触发 env-store 加载', async () => {
    render(<AppShell />);
    expect(mockBackend.listAllEnvVars).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalledTimes(1));
  });

  it('「全部变量」下 PATH 专用按钮不可见', async () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    expect(screen.queryByRole('button', { name: '上移' })).toBeNull();
    expect(screen.queryByRole('button', { name: '下移' })).toBeNull();
    expect(screen.queryByRole('button', { name: '一键清理' })).toBeNull();
    expect(screen.queryByRole('button', { name: '导入' })).toBeNull();
    expect(screen.queryByRole('button', { name: '导出' })).toBeNull();
  });

  it('「全部变量」下渲染环境变量工具栏与表格', async () => {
    mockBackend.listAllEnvVars.mockResolvedValue({
      system: [meta({ name: 'windir', hive: 'system' })],
      user: [meta({ name: 'JAVA_HOME', hive: 'user' })],
    });
    const { container } = render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));

    // 工具栏
    await waitFor(() => expect(screen.getByRole('button', { name: '新建变量' })).not.toBeNull());
    expect(container.querySelector('[data-env-var-key="system:windir"]')).not.toBeNull();
    expect(container.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull();
    expect(screen.getByPlaceholderText('搜索变量名')).not.toBeNull();
    // PATH 表格不得同时存在
    expect(container.querySelector('[data-testid="path-table"]')).toBeNull();
  });

  it('PATH Tab 下环境变量工具栏不可见', () => {
    render(<AppShell />);
    expect(screen.queryByRole('button', { name: '新建变量' })).toBeNull();
  });

  it('切到「合并预览」不触发环境变量加载（PATH 行为不变）', () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('合并预览'));
    expect(mockBackend.listAllEnvVars).not.toHaveBeenCalled();
  });
});

describe('「全部变量」拖放早退（决策 3）', () => {
  it('PATH Tab 下拖入文件夹会新增条目（对照组）', () => {
    const { container } = render(<AppShell />);
    dropFolder(dropZone(container), 'D:\\NewFolder');
    expect(useAppStore.getState().sysPaths.map((entry) => entry.path)).toEqual(['D:\\NewFolder']);
  });

  it('「全部变量」Tab 下拖入文件夹被忽略', async () => {
    const { container } = render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    dropFolder(dropZone(container), 'D:\\NewFolder');
    expect(useAppStore.getState().sysPaths).toEqual([]);
    expect(useAppStore.getState().userPaths).toEqual([]);
  });
});

describe('关窗确认纳入环境变量草稿（决策 4）', () => {
  it('仅有环境变量草稿时也会弹确认，取消则不关窗', () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    // isModified 为 false，草稿是唯一待提交内容。
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'secret']]) });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(confirmSpy).toHaveBeenCalled();
    expect(closeSpy).not.toHaveBeenCalled();
  });

  it('确认后关窗', () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    useAppStore.setState({ isModified: true });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(confirmSpy).toHaveBeenCalled();
    expect(closeSpy).toHaveBeenCalled();
  });

  it('无草稿且未修改时不弹确认，直接关窗（PATH 既有行为）', () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(closeSpy).toHaveBeenCalled();
  });
});

describe('新建环境变量弹窗', () => {
  it('点「新建变量」打开弹窗，确定后调用 createEnvVar', async () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    fireEvent.click(screen.getByRole('button', { name: '新建变量' }));
    fireEvent.change(screen.getByLabelText('变量名'), { target: { value: 'MY_VAR' } });
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'hello' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    await waitFor(() =>
      expect(mockBackend.createEnvVar).toHaveBeenCalledWith('user', 'MY_VAR', 'hello', 'string'),
    );
  });

  it('创建失败时保留弹窗并显示错误', async () => {
    mockBackend.createEnvVar.mockRejectedValue(new Error('变量已存在'));
    const { container } = render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    fireEvent.click(screen.getByRole('button', { name: '新建变量' }));
    fireEvent.change(screen.getByLabelText('变量名'), { target: { value: 'MY_VAR' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    await waitFor(() => expect(screen.getAllByText(/变量已存在/).length).toBeGreaterThan(0));
    expect(screen.getByRole('heading', { name: '新建环境变量' })).not.toBeNull();
    // allVars 下状态栏显示 env-store 的错误消息
    expect(container.querySelector('footer')?.textContent).toContain('变量已存在');
  });

  it('系统 PATH 不可写时禁用新建变量的系统来源', async () => {
    // 该流程中无人调用 getPathCapabilities（能力在 loadPaths 时加载），
    // 直接设置 store 状态更可靠。
    useAppStore.setState({
      isAdmin: false,
      pathCapabilities: {
        canReadSystem: true,
        canWriteSystem: false,
        canReadUser: true,
        canWriteUser: true,
      },
    });
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('button', { name: '新建变量' }));

    expect((screen.getByRole('option', { name: '系统' }) as HTMLOptionElement).disabled).toBe(true);
  });

  it('变量名非法时不调用 createEnvVar 并显示错误', async () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    fireEvent.click(screen.getByRole('button', { name: '新建变量' }));
    fireEvent.change(screen.getByLabelText('变量名'), { target: { value: 'A=B' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    expect(screen.getByText('变量名不能包含等号')).not.toBeNull();
    expect(mockBackend.createEnvVar).not.toHaveBeenCalled();
  });
});

describe('编辑环境变量（选中 → 编辑弹窗 → 保存）', () => {
  async function openEditDialog(): Promise<void> {
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [meta()] });
    // 弹窗打开时经 fetchFullValue（revealEnvVar）取完整原值作为编辑数据源（F-01）
    mockBackend.revealEnvVar.mockResolvedValue('C:\\Java');
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));

    // 先点行选中，工具栏「编辑」才启用
    await waitFor(() =>
      expect(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull(),
    );
    fireEvent.click(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')!);
    await waitFor(() =>
      expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
    fireEvent.click(screen.getByRole('button', { name: '编辑' }));

    // 等完整原值加载完成（输入框从 disabled 变为可用且预填原值）
    const valueInput = screen.getByLabelText('变量值') as HTMLInputElement;
    await waitFor(() => expect(valueInput.disabled).toBe(false));
    expect(valueInput.value).toBe('C:\\Java');
  }

  it('打开弹窗时加载完整原值而非截断 preview（F-01）', async () => {
    // 快照 preview 是 256 字符截断摘要；reveal 返回完整值
    const longFull = 'x'.repeat(300);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [meta()] });
    mockBackend.revealEnvVar.mockResolvedValue(longFull);
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() =>
      expect(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull(),
    );
    fireEvent.click(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')!);
    fireEvent.click(
      screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement /** 已启用 */,
    );

    const valueInput = screen.getByLabelText('变量值') as HTMLInputElement;
    await waitFor(() => expect(valueInput.disabled).toBe(false));
    // 编辑框数据源是完整原值（300 字符），不是 preview
    expect(valueInput.value).toBe(longFull);
    expect(mockBackend.revealEnvVar).toHaveBeenCalledWith('user', 'JAVA_HOME');
  });

  it('确定后调用 updateEnvVar 并携带 revision', async () => {
    mockBackend.updateEnvVar.mockResolvedValue(undefined);
    await openEditDialog();
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'C:\\NewJava' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    await waitFor(() =>
      expect(mockBackend.updateEnvVar).toHaveBeenCalledWith(
        'user',
        'JAVA_HOME',
        'C:\\NewJava',
        'rev-1',
      ),
    );
  });

  it('revision 冲突后刷新，第二次提交携带新 revision 并成功（F-02）', async () => {
    const oldMeta = meta({ revision: 'rev-1' });
    const newMeta = meta({ revision: 'rev-2' });
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [oldMeta] });
    mockBackend.revealEnvVar.mockResolvedValue('C:\\Java');
    // 第一次提交冲突，之后刷新返回新 revision；第二次提交成功
    mockBackend.updateEnvVar
      .mockRejectedValueOnce(new Error('[E_CONFLICT] 变量已被其他进程修改，请重新加载'))
      .mockResolvedValueOnce(undefined);
    mockBackend.listAllEnvVars
      .mockResolvedValueOnce({ system: [], user: [oldMeta] })
      .mockResolvedValue({ system: [], user: [newMeta] });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() =>
      expect(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull(),
    );
    fireEvent.click(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')!);
    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    const valueInput = screen.getByLabelText('变量值') as HTMLInputElement;
    await waitFor(() => expect(valueInput.disabled).toBe(false));

    // 第一次提交：冲突 → 弹窗保留并显示错误
    fireEvent.change(valueInput, { target: { value: 'C:\\NewJava' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));
    await waitFor(() => expect(screen.getAllByText(/已被其他进程修改/).length).toBeGreaterThan(0));
    expect(screen.getByLabelText('变量值')).not.toBeNull();

    // 第二次提交：弹窗未关，直接再点确定 → 自动携带刷新后的 rev-2
    fireEvent.click(screen.getByRole('button', { name: '确定' }));
    await waitFor(() =>
      expect(mockBackend.updateEnvVar).toHaveBeenLastCalledWith(
        'user',
        'JAVA_HOME',
        'C:\\NewJava',
        'rev-2',
      ),
    );
  });

  it('非冲突保存失败时弹窗保留并显示错误', async () => {
    mockBackend.updateEnvVar.mockRejectedValue(
      new Error('[E_CONFLICT] 变量已被其他进程修改，请重新加载'),
    );
    await openEditDialog();
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'C:\\NewJava' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    await waitFor(() => expect(screen.getAllByText(/已被其他进程修改/).length).toBeGreaterThan(0));
    expect(screen.getByLabelText('变量值')).not.toBeNull();
  });

  it('取消后不调用 updateEnvVar', async () => {
    await openEditDialog();
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(mockBackend.updateEnvVar).not.toHaveBeenCalled();
    expect(useEnvStore.getState().draft.size).toBe(0);
  });

  it('编辑中的真实输入镜像进草稿，参与关窗确认（F-04）', async () => {
    await openEditDialog();
    // 打开后草稿为空（只有 fetch 后尚未输入）
    expect(useEnvStore.getState().hasDrafts()).toBe(false);

    // 用户在弹窗中输入 → 草稿实时镜像 → 关窗确认条件成立
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'C:\\Typing' } });
    expect(useEnvStore.getState().hasDrafts()).toBe(true);
    expect(useEnvStore.getState().draft.get('user:JAVA_HOME')).toBe('C:\\Typing');

    // 取消弹窗 → 草稿清除（弹窗内取消不残留确认条件）
    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    expect(useEnvStore.getState().hasDrafts()).toBe(false);
  });
});

describe('删除与选中管理', () => {
  /** 选中 JAVA_HOME 行，使工具栏删除/编辑启用。 */
  async function selectJavaHome(): Promise<void> {
    await waitFor(() =>
      expect(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull(),
    );
    fireEvent.click(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')!);
    await waitFor(() =>
      expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
  }

  it('删除需要确认；确认后调用 deleteEnvVar 并清除选中', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()] });
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [] });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    expect(confirmSpy).toHaveBeenCalled();
    await waitFor(() =>
      expect(mockBackend.deleteEnvVar).toHaveBeenCalledWith('user', 'JAVA_HOME', 'rev-1'),
    );
    await waitFor(() =>
      expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(
        true,
      ),
    );
  });

  it('取消删除时不调用 deleteEnvVar', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()] });
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [] });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    expect(mockBackend.deleteEnvVar).not.toHaveBeenCalled();
    // 选中保留，编辑按钮仍可用
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('刷新后清除指向已不存在变量的选中', async () => {
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()] });
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [] });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));

    await waitFor(() =>
      expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(
        true,
      ),
    );
  });
});
