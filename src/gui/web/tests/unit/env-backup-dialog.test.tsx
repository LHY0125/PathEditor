import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor, cleanup } from '@testing-library/react';

// 走 backend.ts 的唯一 IPC 边界，因此 mock 该模块而不是 invoke：
// 组件/Store/hook 都不得直接 invoke（CLAUDE.md 硬约束）。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  return {
    backend: {
      listEnvBackups: viModule.fn(),
      backupEnvVars: viModule.fn(),
      previewEnvBackup: viModule.fn(),
      restoreEnvBackup: viModule.fn(),
      confirmDialog: viModule.fn(),
    },
  };
});

// i18n 用真实 zh-CN 词条（部分 mock react-i18next），断言基于可见文案而非 key。
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
    if (params) {
      return node.replace(/\{\{(\w+)\}\}/g, (_, k: string) => String(params[k] ?? `{{${k}}}`));
    }
    return node;
  };
  return { ...actual, useTranslation: () => ({ t }) };
});

import { useEnvBackup, buildRestoreConfirm } from '@/components/dialogs/env-backup/use-env-backup';
import { backend } from '@/services/backend';
import type { EnvBackupInfo, RestoreOutcome, RestorePreview } from '@/core/env-backup';

const mockBackend = vi.mocked(backend);

const info: EnvBackupInfo = {
  file: 'env_backup_20260922_120000_000.json',
  path: 'C:\\Users\\me\\.patheditor\\backups\\env_backup_20260922_120000_000.json',
  timestamp: '20260922_120000_000',
  sizeBytes: 2048,
  variableCount: 0,
};

/** 一份含删除项的差异：三个计数两两不等，避免对调测不出。 */
const preview: RestorePreview = {
  changes: [
    { hive: 'user', name: 'NEW_ONLY', kind: 'added' },
    { hive: 'user', name: 'OLD_VAR', kind: 'removed' },
    { hive: 'system', name: 'LEGACY_HOME', kind: 'removed' },
    { hive: 'user', name: 'JAVA_HOME', kind: 'conflict' },
  ],
  added: 1,
  modified: 0,
  removed: 2,
  conflicts: 3,
};

beforeEach(() => {
  vi.resetAllMocks();
  mockBackend.listEnvBackups.mockResolvedValue([info]);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('buildRestoreConfirm 文案组装', () => {
  const labels = {
    deleteWarning: 'D-警告',
    deleteHint: 'D-提示',
    manualFallback: 'M-兜底',
  };

  it('含差异摘要、将删除的变量名与两条手工兜底命令', () => {
    const text = buildRestoreConfirm('INTRO', null, labels);
    expect(text).toContain('M-兜底');
    expect(text).toContain('patheditor backup');
    expect(text).toContain('patheditor env backup');
    expect(text).not.toContain('D-警告');
  });

  it('把删除的变量名逐个列出（不是只给计数）', () => {
    const summary = {
      hasChanges: true,
      hasConflicts: true,
      removedNames: ['user:OLD_VAR', 'system:LEGACY_HOME'],
      text: '新增 1、删除 2、冲突 3',
    };
    const text = buildRestoreConfirm('INTRO', summary, labels);

    expect(text).toContain('user:OLD_VAR');
    expect(text).toContain('system:LEGACY_HOME');
    expect(text).toContain('D-警告');
  });

  it('无删除项时不出现删除警告块', () => {
    const summary = {
      hasChanges: true,
      hasConflicts: false,
      removedNames: [],
      text: '新增 1',
    };
    const text = buildRestoreConfirm('INTRO', summary, labels);

    expect(text).toContain('新增 1');
    expect(text).not.toContain('D-警告');
  });
});

describe('useEnvBackup 恢复确认流程', () => {
  it('选中备份后拉取差异，摘要按计数渲染', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });

    // 断言下发的正是备份路径（路径校验在 Rust 侧，前端只转发）
    expect(mockBackend.previewEnvBackup).toHaveBeenCalledWith(info.path);
    expect(result.current.summary?.text).toContain('新增 1');
    expect(result.current.summary?.text).toContain('删除 2');
    expect(result.current.summary?.text).toContain('冲突 3');
    expect(result.current.summary?.removedNames).toEqual(['user:OLD_VAR', 'system:LEGACY_HOME']);
  });

  it('确认文案含删除项与手工兜底；确认后以 force=false 调用恢复', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.restoreEnvBackup.mockResolvedValue({ applied: 3, skipped: 0, failures: [] });
    mockBackend.confirmDialog.mockResolvedValue(true);
    const onRestored = vi.fn();
    const { result } = renderHook(() => useEnvBackup(true, onRestored));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    const confirmText = mockBackend.confirmDialog.mock.calls[0][0];
    expect(confirmText).toContain('user:OLD_VAR');
    expect(confirmText).toContain('patheditor backup');
    expect(confirmText).toContain('patheditor env backup');
    expect(mockBackend.restoreEnvBackup).toHaveBeenCalledWith(info.path, false);
    expect(onRestored).toHaveBeenCalledTimes(1);
  });

  it('取消确认时不调用恢复', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.confirmDialog.mockResolvedValue(false);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(mockBackend.restoreEnvBackup).not.toHaveBeenCalled();
  });

  it('确认对话框 IPC 失败按取消处理（破坏性操作 fail-closed）', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.confirmDialog.mockRejectedValue(new Error('ipc denied'));
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(mockBackend.restoreEnvBackup).not.toHaveBeenCalled();
  });

  it('code=conflict 时二次确认，同意则 force=true 重试', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    const outcome: RestoreOutcome = { applied: 1, skipped: 0, failures: [] };
    mockBackend.restoreEnvBackup
      .mockRejectedValueOnce({ code: 'conflict', message: '备份后有外部修改' })
      .mockResolvedValueOnce(outcome);
    mockBackend.confirmDialog.mockResolvedValue(true);
    const onRestored = vi.fn();
    const { result } = renderHook(() => useEnvBackup(true, onRestored));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(mockBackend.restoreEnvBackup).toHaveBeenCalledTimes(2);
    expect(mockBackend.restoreEnvBackup).toHaveBeenNthCalledWith(1, info.path, false);
    expect(mockBackend.restoreEnvBackup).toHaveBeenNthCalledWith(2, info.path, true);
    // 第二次确认的文案说明「备份后有外部修改」
    expect(mockBackend.confirmDialog.mock.calls[1][0]).toContain('备份后有外部修改');
    expect(result.current.outcome).toEqual(outcome);
    expect(onRestored).toHaveBeenCalledTimes(1);
  });

  it('code=conflict 时二次确认被拒绝 → 不重试，回到就绪态', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.restoreEnvBackup.mockRejectedValue({ code: 'conflict', message: '冲突' });
    // 第一次（恢复确认）同意，第二次（强制覆盖）拒绝
    mockBackend.confirmDialog.mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(mockBackend.restoreEnvBackup).toHaveBeenCalledTimes(1);
    expect(result.current.outcome).toBeNull();
    expect(result.current.busy).toBe(false);
  });

  it('code=permissionDenied 时提示需要管理员权限（不做任何绕过）', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.restoreEnvBackup.mockRejectedValue({
      code: 'permissionDenied',
      message: '拒绝访问',
    });
    mockBackend.confirmDialog.mockResolvedValue(true);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(result.current.error).toContain('管理员权限');
    // 未提权不得重试/强行覆盖
    expect(mockBackend.restoreEnvBackup).toHaveBeenCalledTimes(1);
  });

  it('预览阶段 permissionDenied 也提示管理员权限，并清掉选中', async () => {
    mockBackend.previewEnvBackup.mockRejectedValue({ code: 'permissionDenied', message: '拒绝' });
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });

    expect(result.current.error).toContain('管理员权限');
    expect(result.current.selected).toBeNull();
  });

  it('立即备份成功后刷新列表并回传路径', async () => {
    mockBackend.backupEnvVars.mockResolvedValue('C:\\b\\env_backup_new.json');
    // 刷新前后列表不同：断言刷新确实发生（否则 createdPath 有值也测不出刷新）
    mockBackend.listEnvBackups
      .mockResolvedValueOnce([info])
      .mockResolvedValue([info, { ...info, file: 'env_backup_new.json' }]);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.createBackup();
    });

    expect(result.current.createdPath).toBe('C:\\b\\env_backup_new.json');
    expect(result.current.backups).toHaveLength(2);
  });

  it('恢复结果含逐条失败时如实保留（不吞掉 failures）', async () => {
    mockBackend.previewEnvBackup.mockResolvedValue(preview);
    mockBackend.restoreEnvBackup.mockResolvedValue({
      applied: 1,
      skipped: 0,
      failures: ['[系统] LEGACY_HOME: 拒绝访问'],
    });
    mockBackend.confirmDialog.mockResolvedValue(true);
    const { result } = renderHook(() => useEnvBackup(true, () => {}));

    await waitFor(() => expect(result.current.backups).toHaveLength(1));
    await act(async () => {
      await result.current.select(info);
    });
    await act(async () => {
      await result.current.restore();
    });

    expect(result.current.outcome?.applied).toBe(1);
    expect(result.current.outcome?.failures).toEqual(['[系统] LEGACY_HOME: 拒绝访问']);
  });

  it('未打开时不拉取备份列表', async () => {
    renderHook(() => useEnvBackup(false, () => {}));

    expect(mockBackend.listEnvBackups).not.toHaveBeenCalled();
  });
});
