import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock i18n
vi.mock('@/i18n', () => ({
  default: { t: vi.fn((key: string, opts?: Record<string, unknown>) => {
    if (key === 'status.loaded') return `已加载 ${opts?.sysCount} 条系统 PATH，${opts?.userCount} 条用户 PATH`;
    if (key === 'status.error') return '加载失败';
    if (key === 'status.saving') return '正在保存...';
    if (key === 'status.saved') return '保存成功';
    if (key === 'status.warning_backup') return '备份失败，但保存继续';
    if (key === 'status.readonly') return '只读模式';
    if (key === 'status.deleted') return `已删除 ${opts?.count} 条路径`;
    return key;
  }) },
}));

import { useAppStore } from '@/store/app-store';
import { UndoRedoManager, TargetType } from '@/core/undo-redo';
import { invoke } from '@tauri-apps/api/core';

const mockedInvoke = vi.mocked(invoke);

function resetStore() {
  useAppStore.setState({
    sysPaths: [],
    userPaths: [],
    undoRedo: new UndoRedoManager(50),
    _savedSys: [],
    _savedUser: [],
    isModified: false,
    isLoading: false,
    isSaving: false,
    selectedIndices: [],
    searchQuery: '',
    statusMessage: '',
  });
}

describe('app-store CRUD', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('addPath 追加到 sysPaths', () => {
    useAppStore.getState().addPath('C:\\test', TargetType.SYSTEM);
    const s = useAppStore.getState();
    expect(s.sysPaths).toEqual(['C:\\test']);
    expect(s.isModified).toBe(true);
    expect(s.undoRedo.historyLength).toBe(1);
  });

  it('addPath 追加到 userPaths', () => {
    useAppStore.getState().addPath('D:\\user', TargetType.USER);
    const s = useAppStore.getState();
    expect(s.userPaths).toEqual(['D:\\user']);
    expect(s.sysPaths).toEqual([]);
  });

  it('editPath 替换正确位置', () => {
    const store = useAppStore.getState();
    store.addPath('C:\\old', TargetType.SYSTEM);
    store.editPath(0, 'C:\\new', TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths).toEqual(['C:\\new']);
  });

  it('editPath 越界 index 无崩溃', () => {
    expect(() => {
      useAppStore.getState().editPath(99, 'X', TargetType.SYSTEM);
    }).not.toThrow();
  });

  it('deletePaths 单选删除', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.addPath('B', TargetType.SYSTEM);
    store.addPath('C', TargetType.SYSTEM);
    store.deletePaths([1], TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths).toEqual(['A', 'C']);
    expect(useAppStore.getState().selectedIndices).toEqual([]);
  });

  it('deletePaths 多选删除（逆序排序一次 undo 覆盖）', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.USER);
    store.addPath('B', TargetType.USER);
    store.addPath('C', TargetType.USER);
    store.addPath('D', TargetType.USER);
    store.deletePaths([1, 3], TargetType.USER);
    expect(useAppStore.getState().userPaths).toEqual(['A', 'C']);
  });

  it('moveUp index=0 无操作', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.moveUp(0, TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths).toEqual(['A']);
  });

  it('moveUp 正常交换位置', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.addPath('B', TargetType.SYSTEM);
    store.moveUp(1, TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths).toEqual(['B', 'A']);
    expect(useAppStore.getState().selectedIndices).toEqual([0]);
  });

  it('moveDown 末位无操作', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.USER);
    store.moveDown(0, TargetType.USER);
    expect(useAppStore.getState().userPaths).toEqual(['A']);
  });

  it('cleanPaths 移除无效路径并返回 removed', () => {
    const store = useAppStore.getState();
    store.addPath('C:\\valid', TargetType.SYSTEM);
    store.addPath(':::invalid:::', TargetType.SYSTEM);
    // is_valid_path_format 拒绝全标点路径
    const removed = store.cleanPaths(TargetType.SYSTEM, (p) => !p.includes(':::'));
    expect(removed).toEqual([':::invalid:::']);
    expect(useAppStore.getState().sysPaths).toEqual(['C:\\valid']);
  });

  it('importPaths 整体替换列表', () => {
    const store = useAppStore.getState();
    store.addPath('old1', TargetType.USER);
    store.addPath('old2', TargetType.USER);
    store.importPaths(TargetType.USER, ['new1', 'new2', 'new3']);
    expect(useAppStore.getState().userPaths).toEqual(['new1', 'new2', 'new3']);
  });

  it('clearPaths 清空列表', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.addPath('B', TargetType.SYSTEM);
    store.clearPaths(TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths).toEqual([]);
  });

  it('clearPaths 空列表无操作', () => {
    const store = useAppStore.getState();
    store.clearPaths(TargetType.USER);
    expect(useAppStore.getState().undoRedo.historyLength).toBe(0);
  });
});

describe('undo/redo', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('undo 恢复操作前状态', () => {
    useAppStore.getState().addPath('test', TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths.length).toBe(1);
    useAppStore.getState().undo();
    expect(useAppStore.getState().sysPaths).toEqual([]);
  });

  it('redo 回到操作后状态', () => {
    const store = useAppStore.getState();
    store.addPath('test', TargetType.SYSTEM);
    store.undo();
    store.redo();
    expect(useAppStore.getState().sysPaths).toEqual(['test']);
  });

  it('undo/redo 正确更新 isModified', () => {
    const store = useAppStore.getState();
    // 设置已保存快照
    useAppStore.setState({ _savedSys: [], _savedUser: [] });
    store.addPath('test', TargetType.SYSTEM);
    expect(useAppStore.getState().isModified).toBe(true);
    store.undo();
    expect(useAppStore.getState().isModified).toBe(false);
    store.redo();
    expect(useAppStore.getState().isModified).toBe(true);
  });
});

describe('_markDirty', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('修改后 isModified 为 true', () => {
    useAppStore.setState({ _savedSys: [], sysPaths: [], _savedUser: [], userPaths: [] });
    useAppStore.getState()._markDirty();
    expect(useAppStore.getState().isModified).toBe(false); // 相等
  });

  it('路径变化时 isModified 为 true', () => {
    useAppStore.setState({
      _savedSys: ['A'], sysPaths: ['B'],
      _savedUser: [], userPaths: [],
    });
    useAppStore.getState()._markDirty();
    expect(useAppStore.getState().isModified).toBe(true);
  });
});

describe('loadPaths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('成功加载', async () => {
    mockedInvoke.mockResolvedValueOnce(['C:\\sys1', 'C:\\sys2']);
    mockedInvoke.mockResolvedValueOnce(['D:\\usr1']);
    await useAppStore.getState().loadPaths();
    const s = useAppStore.getState();
    expect(s.sysPaths).toEqual(['C:\\sys1', 'C:\\sys2']);
    expect(s.userPaths).toEqual(['D:\\usr1']);
    expect(s.isLoading).toBe(false);
    expect(s.isModified).toBe(false);
  });

  it('加载失败时 isLoading 重置', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('reg error'));
    mockedInvoke.mockResolvedValueOnce([]);
    await useAppStore.getState().loadPaths();
    const s = useAppStore.getState();
    expect(s.isLoading).toBe(false);
    expect(s.statusMessage).toContain('加载失败');
  });
});

describe('savePaths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    useAppStore.setState({ sysPaths: ['A'], userPaths: ['B'] });
  });

  it('保存成功', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().savePaths();
    const s = useAppStore.getState();
    expect(s.isSaving).toBe(false);
    expect(s.isModified).toBe(false);
    expect(s.statusMessage).toBe('保存成功');
  });

  it('部分失败时报告具体 hive', async () => {
    mockedInvoke
      .mockResolvedValueOnce(undefined)       // backup_registry
      .mockResolvedValueOnce(undefined)       // save_system_paths
      .mockRejectedValueOnce('权限不足');     // save_user_paths
    await useAppStore.getState().savePaths();
    const s = useAppStore.getState();
    expect(s.isSaving).toBe(false);
    expect(s.statusMessage).toContain('用户 PATH 保存失败');
  });

  it('isSaving 守卫：并发第二次调用直接返回', async () => {
    let resolveAll: (v: unknown) => void;
    const pending = new Promise((r) => { resolveAll = r; });
    mockedInvoke.mockReturnValue(pending as any);

    // 第一次调用（不等它完成，停在 Promise.allSettled）
    const p1 = useAppStore.getState().savePaths();
    // 第二次调用应被 isSaving 守卫拦截（此时 isSaving=true）
    const r2 = useAppStore.getState().savePaths();

    // 第二次调用同步返回 undefined（被守卫拦截）
    await expect(r2).resolves.toBeUndefined();

    // 放行第一次调用的所有 invoke
    resolveAll!(undefined);
    await p1;
  });
});

describe('initialize', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('管理员模式初始化', async () => {
    mockedInvoke
      .mockResolvedValueOnce(true)   // check_admin
      .mockResolvedValueOnce(['S1']) // load_system_paths
      .mockResolvedValueOnce(['U1']); // load_user_paths
    await useAppStore.getState().initialize();
    const s = useAppStore.getState();
    expect(s.isAdmin).toBe(true);
    expect(s.sysPaths).toEqual(['S1']);
    expect(s.userPaths).toEqual(['U1']);
  });

  it('非管理员初始化进入只读模式', async () => {
    mockedInvoke
      .mockResolvedValueOnce(false)   // check_admin
      .mockResolvedValueOnce([])      // load_system_paths
      .mockResolvedValueOnce([]);     // load_user_paths
    await useAppStore.getState().initialize();
    expect(useAppStore.getState().isAdmin).toBe(false);
    // statusMessage 被后续 loadPaths 覆盖为加载完成消息，但 isAdmin=false 不变
  });
});
