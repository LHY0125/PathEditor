import { describe, it, expect, vi, beforeEach } from 'vitest';
import type * as PathSessionModule from '@/services/path-session';
import type { PathEntry } from '../../src/core/path-entry';
import pathCapabilities from '../../tests/fixtures/path-capabilities.json';

vi.mock('@/i18n', () => ({
  default: {
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key} ${JSON.stringify(opts)}` : key,
  },
}));

// 只替换保存编排本身，保留 `arraysEqual` 等纯函数。
vi.mock('@/services/path-session', async (importOriginal) => {
  const actual = await importOriginal<typeof PathSessionModule>();
  return { ...actual, savePathSnapshot: vi.fn() };
});

import { useAppStore } from '@/store/app-store';
import { savePathSnapshot, type SaveSnapshotOutcome } from '@/services/path-session';
import { UndoRedoManager, TargetType } from '@/core/undo-redo';

function pe(path: string, enabled = true): PathEntry {
  return { path, enabled };
}

describe('savePaths 并发编辑', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      sysPaths: [pe('C:\\A')],
      userPaths: [],
      undoRedo: new UndoRedoManager(50),
      _savedSys: [pe('C:\\A')],
      _savedUser: [],
      isModified: false,
      isSaving: false,
      isAdmin: true,
      pathCapabilities: { ...pathCapabilities, canWriteSystem: true },
      selectedIndices: [],
      statusMessage: '',
    });
  });

  it('保存期间的新编辑不会被标记为「已保存」', async () => {
    const save = Promise.withResolvers<SaveSnapshotOutcome>();
    vi.mocked(savePathSnapshot).mockImplementation(() => save.promise);

    const pending = useAppStore.getState().savePaths();

    // 保存还在路上：用户切换了一条路径（长度不变，最容易漏判）
    useAppStore.getState().togglePath(0, TargetType.SYSTEM);
    expect(useAppStore.getState().isModified).toBe(true);

    save.resolve({
      result: { kind: 'success' },
      // 写入注册表的是保存开始时的那份列表
      savedSys: [pe('C:\\A')],
      savedUser: [],
      statusMessage: 'saved',
    });
    await pending;

    // 回归点：旧实现用 await 前的快照计算 isModified，会把它变回 false，
    // 关窗确认随之失效，改动被静默丢弃。
    expect(useAppStore.getState().isModified).toBe(true);
    expect(useAppStore.getState().sysPaths[0].enabled).toBe(false);
    expect(useAppStore.getState()._savedSys[0].enabled).toBe(true);
  });
});
