import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

// Mock i18n
vi.mock('@/i18n', () => ({
  default: {
    t: vi.fn((key: string, opts?: Record<string, unknown>) => {
      if (key === 'status.loaded')
        return `已加载 ${opts?.sysCount} 条系统 PATH，${opts?.userCount} 条用户 PATH`;
      if (key === 'status.error') return '加载失败';
      if (key === 'status.saving') return '正在保存...';
      if (key === 'status.saved') return '保存成功';
      if (key === 'status.saveFailure') return `保存失败: ${opts?.details}`;
      if (key === 'status.saveSystemFailed') return `系统 PATH: ${opts?.error}`;
      if (key === 'status.saveUserFailed') return `用户 PATH: ${opts?.error}`;
      if (key === 'status.warning_backup') return '备份失败，但保存继续';
      if (key === 'status.readonly') return '只读模式';
      if (key === 'status.deleted') return `已删除 ${opts?.count} 条路径`;
      return key;
    }),
  },
}));

import type { PathEntry } from '../../src/core/path-entry';
import pathCapabilities from '../../tests/fixtures/path-capabilities.json';

function pe(s: string, enabled: boolean = true): PathEntry {
  return { path: s, enabled };
}

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
    _pendingSys: null,
    _pendingUser: null,
    isModified: false,
    isLoading: false,
    isSaving: false,
    isAdmin: true,
    pathCapabilities: { ...pathCapabilities, canWriteSystem: true },
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
    expect(s.sysPaths.map((e) => e.path)).toEqual(['C:\\test']);
    expect(s.isModified).toBe(true);
    expect(s.undoRedo.historyLength).toBe(1);
  });

  it('addPath 追加到 userPaths', () => {
    useAppStore.getState().addPath('D:\\user', TargetType.USER);
    const s = useAppStore.getState();
    expect(s.userPaths.map((e) => e.path)).toEqual(['D:\\user']);
    expect(s.sysPaths).toEqual([]);
  });

  it('editPath 替换正确位置', () => {
    const store = useAppStore.getState();
    store.addPath('C:\\old', TargetType.SYSTEM);
    store.editPath(0, 'C:\\new', TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['C:\\new']);
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
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['A', 'C']);
    expect(useAppStore.getState().selectedIndices).toEqual([]);
  });

  it('deletePaths 多选删除（逆序排序一次 undo 覆盖）', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.USER);
    store.addPath('B', TargetType.USER);
    store.addPath('C', TargetType.USER);
    store.addPath('D', TargetType.USER);
    store.deletePaths([1, 3], TargetType.USER);
    expect(useAppStore.getState().userPaths.map((e) => e.path)).toEqual(['A', 'C']);
  });

  it('deletePaths 非连续多选删除后可 undo 恢复到正确位置', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.addPath('B', TargetType.SYSTEM);
    store.addPath('C', TargetType.SYSTEM);
    store.addPath('D', TargetType.SYSTEM);
    store.deletePaths([1, 3], TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['A', 'C']);
    useAppStore.getState().undo();
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['A', 'B', 'C', 'D']);
  });

  it('moveUp index=0 无操作', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.moveUp(0, TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['A']);
  });

  it('moveUp 正常交换位置', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.SYSTEM);
    store.addPath('B', TargetType.SYSTEM);
    store.moveUp(1, TargetType.SYSTEM);
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['B', 'A']);
    expect(useAppStore.getState().selectedIndices).toEqual([0]);
  });

  it('moveDown 末位无操作', () => {
    const store = useAppStore.getState();
    store.addPath('A', TargetType.USER);
    store.moveDown(0, TargetType.USER);
    expect(useAppStore.getState().userPaths.map((e) => e.path)).toEqual(['A']);
  });

  it('cleanPaths 移除无效路径并返回 removed', async () => {
    mockedInvoke.mockResolvedValueOnce([[pe('C:\\valid')], [pe(':::invalid:::')]]);
    const store = useAppStore.getState();
    store.addPath('C:\\valid', TargetType.SYSTEM);
    store.addPath(':::invalid:::', TargetType.SYSTEM);
    const removed = await store.cleanPaths(TargetType.SYSTEM);
    expect(removed).toEqual([':::invalid:::']);
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['C:\\valid']);
  });

  it('replacePaths 整体替换列表', () => {
    const store = useAppStore.getState();
    store.addPath('old1', TargetType.USER);
    store.addPath('old2', TargetType.USER);
    store.replacePaths(TargetType.USER, [pe('new1'), pe('new2'), pe('new3')]);
    expect(useAppStore.getState().userPaths.map((e) => e.path)).toEqual(['new1', 'new2', 'new3']);
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
    expect(useAppStore.getState().sysPaths.map((e) => e.path)).toEqual(['test']);
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

describe('loadPaths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it('成功加载', async () => {
    mockedInvoke.mockResolvedValueOnce({
      system: [pe('C:\\sys1'), pe('C:\\sys2')],
      user: [pe('D:\\usr1')],
    });
    await useAppStore.getState().loadPaths();
    const s = useAppStore.getState();
    expect(s.sysPaths.map((e) => e.path)).toEqual(['C:\\sys1', 'C:\\sys2']);
    expect(s.userPaths.map((e) => e.path)).toEqual(['D:\\usr1']);
    expect(s.isLoading).toBe(false);
    expect(s.isModified).toBe(false);
  });

  it('加载失败时 isLoading 重置', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('reg error'));
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
    useAppStore.setState({ sysPaths: [pe('A')], userPaths: [pe('B')] });
  });

  it('保存成功', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const result = await useAppStore.getState().savePaths();
    expect(result).toEqual({ kind: 'success' });
    const s = useAppStore.getState();
    expect(s.isSaving).toBe(false);
    expect(s.isModified).toBe(false);
    expect(s.statusMessage).toBe('保存成功');
  });

  it('部分失败时报告具体 hive 并保留草稿', async () => {
    mockedInvoke
      .mockResolvedValueOnce(undefined) // backup_registry
      .mockResolvedValueOnce(undefined) // save_system_paths
      .mockRejectedValueOnce('权限不足') // save_user_paths
      .mockResolvedValueOnce(undefined); // save_path_snapshot

    const result = await useAppStore.getState().savePaths();
    expect(result.kind).toBe('partial');
    const s = useAppStore.getState();
    expect(s.isSaving).toBe(false);
    expect(s.statusMessage).toContain('用户 PATH');
    expect(s.userPaths.map((entry) => entry.path)).toEqual(['B']);
    expect(s.isModified).toBe(true);
  });

  it('禁用状态写入失败后保留 dirty，并在重试时只补写 sidecar', async () => {
    mockedInvoke
      .mockResolvedValueOnce(undefined) // backup_registry
      .mockResolvedValueOnce(undefined) // save_system_paths
      .mockResolvedValueOnce(undefined) // save_user_paths
      .mockResolvedValueOnce(undefined) // broadcast_env_change
      .mockRejectedValueOnce('磁盘写入失败'); // save_path_snapshot

    const first = await useAppStore.getState().savePaths();
    expect(first.kind).toBe('partial');
    let state = useAppStore.getState();
    expect(state.isModified).toBe(true);
    expect(state._pendingSys?.map((entry) => entry.path)).toEqual(['A']);
    expect(state._pendingUser?.map((entry) => entry.path)).toEqual(['B']);

    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValueOnce(undefined); // 只重试 save_path_snapshot

    const second = await useAppStore.getState().savePaths();
    expect(second).toEqual({ kind: 'success' });
    state = useAppStore.getState();
    expect(state.isModified).toBe(false);
    expect(state._pendingSys).toBeNull();
    expect(state._pendingUser).toBeNull();
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke.mock.calls[0][0]).toBe('save_path_snapshot');
  });
  it('isSaving 守卫：并发第二次调用直接返回', async () => {
    let resolveAll: (v: unknown) => void;
    const pending = new Promise((r) => {
      resolveAll = r;
    });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    mockedInvoke.mockReturnValue(pending as any);

    // 第一次调用（不等它完成，停在 Promise.allSettled）
    const p1 = useAppStore.getState().savePaths();
    // 第二次调用应被 isSaving 守卫拦截（此时 isSaving=true）
    const r2 = useAppStore.getState().savePaths();

    // 第二次调用同步返回 blocked（被守卫拦截）
    await expect(r2).resolves.toEqual({ kind: 'blocked' });

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
      .mockResolvedValueOnce({
        canReadSystem: true,
        canWriteSystem: true,
        canReadUser: true,
        canWriteUser: true,
      }) // get_path_capabilities
      .mockResolvedValueOnce({ system: [pe('S1')], user: [pe('U1')] }); // load_path_snapshot
    await useAppStore.getState().initialize();
    const s = useAppStore.getState();
    expect(s.isAdmin).toBe(true);
    expect(s.sysPaths.map((e) => e.path)).toEqual(['S1']);
    expect(s.userPaths.map((e) => e.path)).toEqual(['U1']);
  });

  it('非管理员初始化进入只读模式', async () => {
    mockedInvoke
      .mockResolvedValueOnce(pathCapabilities) // get_path_capabilities
      .mockResolvedValueOnce({ system: [], user: [] }); // load_path_snapshot
    await useAppStore.getState().initialize();
    expect(useAppStore.getState().isAdmin).toBe(false);
    // statusMessage 被后续 loadPaths 覆盖为加载完成消息，但 isAdmin=false 不变
  });
});
