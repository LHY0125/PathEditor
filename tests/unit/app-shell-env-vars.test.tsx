import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// 异步工厂 + vi.mocked(backend)：与 env-var-table.test.tsx / env-store.test.ts 一致。
// 不能在工厂外引用 mock 变量 —— vi.mock 会被提升到 import 之前，外层 const 仍处于 TDZ。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  // confirmDialog 委托给 plugin-dialog 的 confirm mock：与真实 backend.confirmDialog
  // 的转发关系一致，断言 dialogConfirm 即覆盖完整链路。
  const { confirm: tauriConfirm } = await import('@tauri-apps/plugin-dialog');
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
      confirmDialog: (message: string) => tauriConfirm(message),
    },
  };
});

// 关窗/删除确认走 Tauri 异步对话框（plugin-dialog），mock 掉避免 jsdom 下走真实 IPC。
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

// i18n 用**部分 mock**：保留 initReactI18next 等真实导出（src/i18n 在模块加载时要用），
// 只覆盖 useTranslation，并以真实 zh-CN 词条取值，断言基于可见文案而非 i18n key。
vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  const zh = ((await import('@/i18n/locales/zh-CN.json')).default ?? {}) as Record<string, unknown>;
  const t = (key: string, params?: Record<string, unknown>): string => {
    let node: unknown = zh;
    for (const part of key.split('.')) {
      if (node === null || typeof node !== 'object') return key;
      node = (node as Record<string, unknown>)[part];
    }
    if (typeof node !== 'string') return key;
    // 简单 {{name}} 插值：confirmDialog 收到的是渲染后的完整文案。
    if (params) {
      return node.replace(/\{\{(\w+)\}\}/g, (_, k: string) => String(params[k] ?? `{{${k}}}`));
    }
    return node;
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
import { confirm as dialogConfirm } from '@tauri-apps/plugin-dialog';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

const mockBackend = vi.mocked(backend);

/** 设定异步确认对话框的返回值（关窗确认 describe 与后续用例共用）。 */
function mockDialogConfirm(v: boolean) {
  return vi.mocked(dialogConfirm).mockResolvedValue(v);
}

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
  mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });
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
  useEnvStore.setState({ draft: new Map(), snapshot: { system: [], user: [], capturedAt: 0 } });
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
      capturedAt: 0,
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

describe('关窗确认纳入环境变量草稿（异步对话框版）', () => {
  it('仅有环境变量草稿时也弹异步确认，取消则不关窗', async () => {
    mockDialogConfirm(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    // isModified 为 false，草稿是唯一待提交内容。
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'secret']]) });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    await waitFor(() => expect(dialogConfirm).toHaveBeenCalledWith('有未保存的修改，确定退出吗？'));
    expect(closeSpy).not.toHaveBeenCalled();
  });

  it('确认后关窗', async () => {
    mockDialogConfirm(true);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    useAppStore.setState({ isModified: true });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    await waitFor(() => expect(dialogConfirm).toHaveBeenCalled());
    expect(closeSpy).toHaveBeenCalled();
  });

  it('无草稿且未修改时不弹确认，直接关窗（PATH 既有行为）', async () => {
    mockDialogConfirm(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(dialogConfirm).not.toHaveBeenCalled();
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
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [meta()], capturedAt: 0 });
    // 弹窗打开时经 fetchFullValue（revealEnvVar）取完整原值作为编辑数据源（F-01）；
    // 返回值携带读取时 revision，供保存点陈旧判定。
    mockBackend.revealEnvVar.mockResolvedValue({ value: 'C:\\Java', revision: 'rev-1' });
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
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [meta()], capturedAt: 0 });
    mockBackend.revealEnvVar.mockResolvedValue({ value: longFull, revision: 'rev-1' });
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

  it('revision 冲突后刷新，重取+再次提交携带新 revision 并成功（F-02）', async () => {
    const oldMeta = meta({ revision: 'rev-1' });
    const newMeta = meta({ revision: 'rev-2' });
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [oldMeta], capturedAt: 0 });
    // reveal 返回的 revision 跟随当前快照：模拟后端按注册表现状下发读取时版本
    mockBackend.revealEnvVar.mockImplementation(async () => ({
      value: 'C:\\Java',
      revision:
        useEnvStore.getState().snapshot?.user.find((m) => m.name === 'JAVA_HOME')?.revision ??
        'rev-1',
    }));
    // 第一次提交冲突，之后刷新返回新 revision；重取后再提交成功
    mockBackend.updateEnvVar
      .mockRejectedValueOnce({
        code: 'conflict',
        operation: 'update_env_var',
        hive: 'user',
        name: 'JAVA_HOME',
        retryable: true,
        message: '变量已被其他进程修改，请重新加载',
      })
      .mockResolvedValueOnce(undefined);
    mockBackend.listAllEnvVars
      .mockResolvedValueOnce({ system: [], user: [oldMeta], capturedAt: 0 })
      .mockResolvedValue({ system: [], user: [newMeta], capturedAt: 0 });

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

    // 第二次提交：弹窗读值绑定 rev-1 而快照已是 rev-2 → 陈旧重取，本次不保存
    fireEvent.click(screen.getByRole('button', { name: '确定' }));
    await waitFor(() => expect(screen.getAllByText(/已被外部修改/).length).toBeGreaterThan(0));
    expect(mockBackend.updateEnvVar).toHaveBeenCalledTimes(1);
    // 输入框已被重取的最新原值替换；用户重新输入后再提交
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'C:\\NewJava' } });

    // 第三次提交：读值 revision 已随重取换代 → 携带 rev-2 成功
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

  it('快照刷新后提交旧值：重取最新值且不调用 updateEnvVar（F-01）', async () => {
    const oldMeta = meta({ revision: 'rev-1' });
    const newMeta = meta({ revision: 'rev-2' });
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [oldMeta], capturedAt: 0 });
    // 第一次取值绑定 rev-1；陈旧重取时返回绑定 rev-2 的最新值
    mockBackend.revealEnvVar
      .mockResolvedValueOnce({ value: 'C:\\Java', revision: 'rev-1' })
      .mockResolvedValue({ value: 'C:\\JavaFresh', revision: 'rev-2' });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() =>
      expect(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull(),
    );
    fireEvent.click(document.querySelector('[data-env-var-key="user:JAVA_HOME"]')!);
    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    const valueInput = screen.getByLabelText('变量值') as HTMLInputElement;
    await waitFor(() => expect(valueInput.disabled).toBe(false));
    expect(valueInput.value).toBe('C:\\Java');

    // 弹窗打开期间快照换代（外部进程修改 → 刷新后 revision 变化）
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [newMeta], capturedAt: 0 });
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(useEnvStore.getState().snapshot?.user[0]?.revision).toBe('rev-2'));

    // 提交旧值 → 弹窗检测读取版本已陈旧：重取最新值，本次不保存
    fireEvent.click(screen.getByRole('button', { name: '确定' }));
    await waitFor(() => expect(screen.getAllByText(/已被外部修改/).length).toBeGreaterThan(0));
    expect(mockBackend.updateEnvVar).not.toHaveBeenCalled();
    // 输入框已被重取的最新值替换，用户确认后可再次提交
    expect((screen.getByLabelText('变量值') as HTMLInputElement).value).toBe('C:\\JavaFresh');
  });

  it('非冲突保存失败时弹窗保留并显示错误', async () => {
    mockBackend.updateEnvVar.mockRejectedValue({
      code: 'protected',
      operation: 'update_env_var',
      hive: 'user',
      name: 'JAVA_HOME',
      retryable: false,
      message: 'JAVA_HOME 是系统内置变量，不允许修改',
    });
    await openEditDialog();
    fireEvent.change(screen.getByLabelText('变量值'), { target: { value: 'C:\\NewJava' } });
    fireEvent.click(screen.getByRole('button', { name: '确定' }));

    await waitFor(() => expect(screen.getAllByText(/不允许修改/).length).toBeGreaterThan(0));
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

  it('删除前弹异步确认；确认后调用 deleteEnvVar 并清除选中', async () => {
    mockDialogConfirm(true);
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()], capturedAt: 0 });
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    await waitFor(() =>
      expect(dialogConfirm).toHaveBeenCalledWith('确定删除变量 JAVA_HOME 吗？此操作不可撤销。'),
    );
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
    mockDialogConfirm(false);
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()], capturedAt: 0 });
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    await waitFor(() =>
      expect(dialogConfirm).toHaveBeenCalledWith('确定删除变量 JAVA_HOME 吗？此操作不可撤销。'),
    );
    expect(mockBackend.deleteEnvVar).not.toHaveBeenCalled();
    // 选中保留，编辑按钮仍可用
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('确认对话框 IPC 失败时不删除（破坏性操作 fail-closed，与关窗路径相反）', async () => {
    vi.mocked(dialogConfirm).mockRejectedValue(new Error('ipc denied'));
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()], capturedAt: 0 });
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });

    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await selectJavaHome();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    await waitFor(() => expect(dialogConfirm).toHaveBeenCalled());
    // IPC 失败按取消处理：绝不静默删除
    expect(mockBackend.deleteEnvVar).not.toHaveBeenCalled();
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('刷新后清除指向已不存在变量的选中', async () => {
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [meta()], capturedAt: 0 });
    mockBackend.listAllEnvVars.mockResolvedValueOnce({ system: [], user: [], capturedAt: 0 });

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
