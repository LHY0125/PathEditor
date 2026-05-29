import { describe, it, expect, vi, beforeEach } from 'vitest';

// ── Mock 外部依赖 ──

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

const mockOpen = vi.fn().mockResolvedValue(null);
const mockAsk = vi.fn().mockResolvedValue(true);
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (...args: unknown[]) => mockOpen(...args),
  ask: (...args: unknown[]) => mockAsk(...args),
}));

vi.mock('@/i18n', () => ({
  default: { t: vi.fn((key: string, opts?: Record<string, unknown>) => {
    if (key === 'status.deleted') return `已删除 ${opts?.count} 条`;
    if (key === 'status.saveWarningLongPaths') return 'PATH 长度超限';
    return key;
  }) },
}));

vi.mock('@/hooks/use-keyboard', () => ({
  useKeyboard: vi.fn(),
}));

import { renderHook, act } from '@testing-library/react';
import { useAppStore } from '@/store/app-store';
import { UndoRedoManager, TargetType } from '@/core/undo-redo';
import { invoke } from '@tauri-apps/api/core';
import type { PathEntry } from '@/core/path-entry';

const mockedInvoke = vi.mocked(invoke);

function pe(s: string, enabled = true): PathEntry {
  return { path: s, enabled };
}

function resetStore(sys: PathEntry[] = [], user: PathEntry[] = []) {
  useAppStore.setState({
    sysPaths: sys,
    userPaths: user,
    undoRedo: new UndoRedoManager(50),
    _savedSys: sys,
    _savedUser: user,
    selectedIndices: [],
    isModified: false,
    isLoading: false,
    isSaving: false,
    isAdmin: true,
    statusMessage: '',
  });
}

// useAppActions 需要 DialogState，创建一个 mock
function mockDialogs() {
  return {
    editDialog: { open: false, index: -1, value: '', target: TargetType.SYSTEM },
    newDialog: false,
    helpOpen: false,
    importDialog: { open: false, system: [] as PathEntry[], user: [] as PathEntry[] },
    setEditDialog: vi.fn(),
    setNewDialog: vi.fn(),
    setHelpOpen: vi.fn(),
    setImportDialog: vi.fn(),
    setAnalyzeOpen: vi.fn(),
    setProfilesOpen: vi.fn(),
  };
}

describe('useAppActions', () => {
  let dialogs: ReturnType<typeof mockDialogs>;

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore([pe('C:\\Windows'), pe('C:\\Program Files')], [pe('D:\\User')]);
    dialogs = mockDialogs();
  });

  // ── handleNew ──

  it('handleNew 打开新建弹窗', async () => {
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleNew(); });
    expect(dialogs.setNewDialog).toHaveBeenCalledWith(true);
  });

  // ── handleEdit ──

  it('handleEdit 打开编辑弹窗（有选中项）', async () => {
    useAppStore.setState({ selectedIndices: [0] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleEdit(); });
    expect(dialogs.setEditDialog).toHaveBeenCalledWith({
      open: true, index: 0, value: 'C:\\Windows', target: TargetType.SYSTEM,
    });
  });

  it('handleEdit 无选中项不操作', async () => {
    useAppStore.setState({ selectedIndices: [] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleEdit(); });
    expect(dialogs.setEditDialog).not.toHaveBeenCalled();
  });

  // ── handleDelete ──

  it('handleDelete 删除选中项', async () => {
    useAppStore.setState({ selectedIndices: [0] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleDelete(); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\Program Files']);
  });

  it('handleDelete 无选中项不操作', async () => {
    useAppStore.setState({ selectedIndices: [] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleDelete(); });
    expect(useAppStore.getState().sysPaths.length).toBe(2);
  });

  // ── handleMoveUp / handleMoveDown ──

  it('handleMoveUp 上移选中项', async () => {
    useAppStore.setState({ selectedIndices: [1] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleMoveUp(); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  it('handleMoveDown 下移选中项', async () => {
    useAppStore.setState({ selectedIndices: [0] });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleMoveDown(); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  // ── handleClean ──

  it('handleClean 清理无效路径', async () => {
    resetStore([pe('C:\\Windows'), pe('invalid_path!@#')]);
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleClean(); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\Windows']);
    expect(useAppStore.getState().statusMessage).toContain('已删除 1 条');
  });

  // ── handleNewConfirm ──

  it('handleNewConfirm 添加新路径', async () => {
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleNewConfirm('C:\\New'); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toContain('C:\\New');
    expect(dialogs.setNewDialog).toHaveBeenCalledWith(false);
  });

  it('handleNewConfirm 空白不添加', async () => {
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleNewConfirm('  '); });
    expect(useAppStore.getState().sysPaths.length).toBe(2);
  });

  // ── handleEditConfirm ──

  it('handleEditConfirm 修改路径', async () => {
    dialogs.editDialog = { open: true, index: 0, value: 'C:\\Windows', target: TargetType.SYSTEM };
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleEditConfirm('C:\\Edited'); });
    expect(useAppStore.getState().sysPaths[0].path).toBe('C:\\Edited');
  });

  // ── handleImportSelect ──

  it('handleImportSelect both 模式替换双 hive', async () => {
    const sysImport = [pe('C:\\ImportSys')];
    const usrImport = [pe('D:\\ImportUsr')];
    dialogs.importDialog = { open: true, system: sysImport, user: usrImport };
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleImportSelect('both'); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\ImportSys']);
    expect(useAppStore.getState().userPaths.map(e => e.path)).toEqual(['D:\\ImportUsr']);
    expect(dialogs.setImportDialog).toHaveBeenCalledWith({ open: false, system: [], user: [] });
  });

  it('handleImportSelect system 模式只替换 system', async () => {
    dialogs.importDialog = { open: true, system: [pe('C:\\ImportSys')], user: [pe('D:\\ImportUsr')] };
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    act(() => { result.current.handleImportSelect('system'); });
    expect(useAppStore.getState().sysPaths.map(e => e.path)).toEqual(['C:\\ImportSys']);
    expect(useAppStore.getState().userPaths.map(e => e.path)).toEqual(['D:\\User']); // 未变
  });

  // ── handleSave ──

  it('handleSave 正常保存', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    await act(async () => { await result.current.handleSave(); });
    // invoke 被调用（backup + save_system + save_user + broadcast）
    expect(mockedInvoke).toHaveBeenCalled();
  });

  it('handleSave 超长确认后强制保存', async () => {
    // 第一次 savePaths 返回 false（超长）
    // 第二次（force=true）返回 true
    let callCount = 0;
    vi.spyOn(useAppStore.getState(), 'savePaths').mockImplementation(async (force?: boolean) => {
      callCount++;
      if (!force) return false; // 第一次：超长警告
      return true; // 第二次：强制保存成功
    });
    const { useAppActions } = await import('@/hooks/use-app-actions');
    const { result } = renderHook(() => useAppActions('system', dialogs));
    await act(async () => { await result.current.handleSave(); });
    expect(callCount).toBe(2);
    expect(mockAsk).toHaveBeenCalled();
  });
});
