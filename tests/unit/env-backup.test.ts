import { describe, it, expect, vi, beforeEach } from 'vitest';

// backend.ts 的 env 写方法返回值契约：IPC 返回的 `WriteOutcome` 形状不可信，
// 必须做运行时校验（与 parsePathCapabilities 同思路）。直接 mock invoke 注入恶意形状。
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// 差异摘要用例不依赖 IPC，但同一文件引入 backend —— 顺带锁定其形状校验行为。
import { invoke } from '@tauri-apps/api/core';
import { backend } from '@/services/backend';
import { backupFailed, type BackupOutcome } from '@/core/env-var';
import { formatSize, summarizePreview, type RestorePreview } from '@/core/env-backup';

const mockInvoke = vi.mocked(invoke);

function preview(over: Partial<RestorePreview> = {}): RestorePreview {
  return { changes: [], added: 0, modified: 0, removed: 0, conflicts: 0, ...over };
}

beforeEach(() => {
  vi.resetAllMocks();
});

describe('备份差异摘要', () => {
  it('无差异时返回「无变化」文案', () => {
    const s = summarizePreview(preview());

    expect(s.hasChanges).toBe(false);
    expect(s.hasConflicts).toBe(false);
    expect(s.removedNames).toEqual([]);
    expect(s.text).toContain('无变化');
  });

  it('按新增/修改/删除分别计数并拼接文案（三个计数两两不等，避免对调测不出）', () => {
    const s = summarizePreview(preview({ added: 2, modified: 7, removed: 3 }));

    expect(s.hasChanges).toBe(true);
    expect(s.text).toContain('新增 2');
    expect(s.text).toContain('修改 7');
    expect(s.text).toContain('删除 3');
    // 反向断言：计数不得串位（把 added 渲染成 modified 会命中这里）
    expect(s.text).not.toContain('新增 7');
    expect(s.text).not.toContain('删除 7');
    expect(s.text).not.toContain('修改 2');
  });

  it('只有冲突时 hasChanges 仍为 false，但文案显式提示冲突', () => {
    const s = summarizePreview(preview({ conflicts: 5 }));

    expect(s.hasConflicts).toBe(true);
    expect(s.hasChanges).toBe(false);
    expect(s.text).toContain('冲突 5');
  });

  it('同时有变更与冲突时文案带「中止」提示', () => {
    const s = summarizePreview(preview({ added: 1, conflicts: 1 }));

    expect(s.text).toContain('新增 1');
    expect(s.text).toContain('冲突 1');
    expect(s.text).toContain('中止');
  });

  it('列出将被删除的变量名——这是最不可逆的部分', () => {
    const s = summarizePreview(
      preview({
        removed: 2,
        added: 1,
        changes: [
          { hive: 'user', name: 'NEW_VAR', kind: 'added' },
          { hive: 'user', name: 'OLD_VAR', kind: 'removed' },
          { hive: 'system', name: 'LEGACY_HOME', kind: 'removed' },
        ],
      }),
    );

    // 只列 removed，且保留 hive 区分（同名变量可能两个 hive 都有）
    expect(s.removedNames).toEqual(['user:OLD_VAR', 'system:LEGACY_HOME']);
  });

  it('modified 恒为 0 时按真实计数如实渲染，不做换算', () => {
    // Task 5 裁断：revision 取值使「值变了」≡「备份过期」→ 一律计 conflict。
    // 前端不得把 conflict 折算成 modified（那是复制判定逻辑）。
    const s = summarizePreview(preview({ conflicts: 4 }));

    expect(s.text).not.toContain('修改 4');
    expect(s.text).not.toContain('修改');
    expect(s.text).toContain('冲突 4');
  });
});

describe('backupFailed 形状判定（成对用例）', () => {
  it('{failed:"原因"} → 返回原因', () => {
    expect(backupFailed({ failed: '磁盘已满' })).toBe('磁盘已满');
  });

  it('{created:"路径"} → 非失败，返回 null', () => {
    expect(backupFailed({ created: 'C:\\b\\env_backup_x.json' })).toBeNull();
  });

  it("'skipped' → 非失败，返回 null", () => {
    expect(backupFailed('skipped')).toBeNull();
  });

  it('未知形状按「非失败」处理，不影响写入成功语义', () => {
    // 未来新增变体（如 'noop' 字符串或 { skipped: true }）不得被误判为失败
    expect(backupFailed('noop' as unknown as BackupOutcome)).toBeNull();
    expect(backupFailed({} as unknown as BackupOutcome)).toBeNull();
    expect(backupFailed(null as unknown as BackupOutcome)).toBeNull();
  });

  it('{failed} 与 {created} 同形不同键时不得混淆（成对反例）', () => {
    expect(backupFailed({ failed: 'x' })).toBe('x');
    expect(backupFailed({ created: 'x' })).toBeNull();
  });
});

describe('formatSize', () => {
  it('按 B / KB / MB 分档', () => {
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(2048)).toBe('2.0 KB');
    expect(formatSize(3 * 1024 * 1024)).toBe('3.0 MB');
  });
});

describe('backend 写方法返回 WriteOutcome 的运行时形状校验', () => {
  it('合法 {backup:{created}} 通过并白名单构造', async () => {
    mockInvoke.mockResolvedValue({ backup: { created: 'C:\\b\\env_backup_1.json' } });

    const outcome = await backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1');

    expect(outcome).toEqual({ backup: { created: 'C:\\b\\env_backup_1.json' } });
  });

  it("合法 'skipped' 与 {failed} 通过", async () => {
    mockInvoke.mockResolvedValue({ backup: 'skipped' });
    await expect(backend.createEnvVar('user', 'N', 'v', 'string')).resolves.toEqual({
      backup: 'skipped',
    });

    mockInvoke.mockResolvedValue({ backup: { failed: '磁盘满' } });
    await expect(backend.deleteEnvVar('user', 'N', 'rev')).resolves.toEqual({
      backup: { failed: '磁盘满' },
    });
  });

  it('undefined（旧后端 / mock 未接线）→ 规范化为 skipped，不误报备份失败', async () => {
    mockInvoke.mockResolvedValue(undefined);

    await expect(backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1')).resolves.toEqual({
      backup: 'skipped',
    });
  });

  it('缺少 backup 字段时拒绝（形状校验删除后本用例会失败）', async () => {
    mockInvoke.mockResolvedValue({ applied: 1 });

    await expect(backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1')).rejects.toThrow(
      /WriteOutcome/,
    );
  });

  it('backup 为非法形状（未知键）时拒绝', async () => {
    mockInvoke.mockResolvedValue({ backup: { something: 'else' } });

    await expect(backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1')).rejects.toThrow(
      /WriteOutcome/,
    );
  });

  it('backup.created 非字符串时拒绝', async () => {
    mockInvoke.mockResolvedValue({ backup: { created: 42 } });

    await expect(backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1')).rejects.toThrow(
      /WriteOutcome/,
    );
  });

  it('白名单构造：上游多余字段不透传', async () => {
    mockInvoke.mockResolvedValue({ backup: 'skipped', extra: 'payload' });

    const outcome = await backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1');

    expect(Object.keys(outcome)).toEqual(['backup']);
  });
});

/** 契约合法的 RestorePreview；各用例按需覆盖单个字段。 */
function rawPreview(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    changes: [{ hive: 'user', name: 'JAVA_HOME', kind: 'conflict' }],
    added: 1,
    modified: 0,
    removed: 2,
    conflicts: 3,
    ...over,
  };
}

describe('备份/恢复命令返回值的运行时形状校验', () => {
  it('previewEnvBackup：合法值白名单构造（多余字段丢弃）', async () => {
    mockInvoke.mockResolvedValue(rawPreview({ extra: 'payload' }));

    const result = await backend.previewEnvBackup('C:\\b\\env_backup_1.json');

    expect(result).toEqual({
      changes: [{ hive: 'user', name: 'JAVA_HOME', kind: 'conflict' }],
      added: 1,
      modified: 0,
      removed: 2,
      conflicts: 3,
    });
  });

  it('previewEnvBackup：转发 file 参数（不做任何路径判定）', async () => {
    mockInvoke.mockResolvedValue(rawPreview());

    await backend.previewEnvBackup('C:\\b\\env_backup_1.json');

    expect(mockInvoke).toHaveBeenCalledWith('preview_env_backup', {
      file: 'C:\\b\\env_backup_1.json',
    });
  });

  it('previewEnvBackup：changes 非数组时拒绝', async () => {
    mockInvoke.mockResolvedValue(rawPreview({ changes: 'nope' }));
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/RestorePreview/);
  });

  it('previewEnvBackup：差异项的 hive 非白名单值时拒绝', async () => {
    mockInvoke.mockResolvedValue(
      rawPreview({ changes: [{ hive: 'machine', name: 'X', kind: 'added' }] }),
    );
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/RestoreChange/);
  });

  it('previewEnvBackup：差异项的 kind 非白名单值时拒绝', async () => {
    mockInvoke.mockResolvedValue(
      rawPreview({ changes: [{ hive: 'user', name: 'X', kind: 'renamed' }] }),
    );
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/RestoreChange/);
  });

  it('previewEnvBackup：差异项 name 为空时拒绝', async () => {
    mockInvoke.mockResolvedValue(
      rawPreview({ changes: [{ hive: 'user', name: '', kind: 'added' }] }),
    );
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/RestoreChange/);
  });

  it('previewEnvBackup：计数为负数或非整数时拒绝', async () => {
    mockInvoke.mockResolvedValue(rawPreview({ removed: -1 }));
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/计数/);

    mockInvoke.mockResolvedValue(rawPreview({ conflicts: 1.5 }));
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/计数/);

    mockInvoke.mockResolvedValue(rawPreview({ added: '3' }));
    await expect(backend.previewEnvBackup('x')).rejects.toThrow(/计数/);
  });

  it('listEnvBackups：合法列表白名单构造，variableCount 缺失回退 0', async () => {
    mockInvoke.mockResolvedValue([
      {
        file: 'env_backup_1.json',
        path: 'C:\\b\\env_backup_1.json',
        timestamp: '20260922_120000_000',
        sizeBytes: 2048,
      },
    ]);

    const list = await backend.listEnvBackups();

    expect(list).toEqual([
      {
        file: 'env_backup_1.json',
        path: 'C:\\b\\env_backup_1.json',
        timestamp: '20260922_120000_000',
        sizeBytes: 2048,
        variableCount: 0,
      },
    ]);
  });

  it('listEnvBackups：非数组或字段缺失时拒绝', async () => {
    mockInvoke.mockResolvedValue({ file: 'x' });
    await expect(backend.listEnvBackups()).rejects.toThrow(/EnvBackupInfo/);

    mockInvoke.mockResolvedValue([{ file: 'x', path: 'y', timestamp: 'z' }]);
    await expect(backend.listEnvBackups()).rejects.toThrow(/EnvBackupInfo/);
  });

  it('backupEnvVars：返回空字符串时拒绝（成功路径必须有路径）', async () => {
    mockInvoke.mockResolvedValue('');
    await expect(backend.backupEnvVars()).rejects.toThrow(/备份路径/);

    mockInvoke.mockResolvedValue(null);
    await expect(backend.backupEnvVars()).rejects.toThrow(/备份路径/);
  });

  it('backupEnvVars：合法路径原样返回', async () => {
    mockInvoke.mockResolvedValue('C:\\b\\env_backup_1.json');
    await expect(backend.backupEnvVars()).resolves.toBe('C:\\b\\env_backup_1.json');
  });

  it('restoreEnvBackup：转发 file 与 force', async () => {
    mockInvoke.mockResolvedValue({ applied: 1, skipped: 0, failures: [] });

    await backend.restoreEnvBackup('C:\\b\\env_backup_1.json', true);

    expect(mockInvoke).toHaveBeenCalledWith('restore_env_backup', {
      file: 'C:\\b\\env_backup_1.json',
      force: true,
    });
  });

  it('restoreEnvBackup：failures 含非字符串项时拒绝', async () => {
    mockInvoke.mockResolvedValue({ applied: 1, skipped: 0, failures: [42] });
    await expect(backend.restoreEnvBackup('x', false)).rejects.toThrow(/failures/);
  });

  it('restoreEnvBackup：计数非法时拒绝', async () => {
    mockInvoke.mockResolvedValue({ applied: '1', skipped: 0, failures: [] });
    await expect(backend.restoreEnvBackup('x', false)).rejects.toThrow(/计数/);
  });

  it('rejection 仍走 parseCoreError：code 结构化透传（新增命令同样适用）', async () => {
    mockInvoke.mockRejectedValue({ code: 'permissionDenied', message: '拒绝访问' });

    const err = await backend.restoreEnvBackup('x', false).catch((e) => e);

    expect((err as { code: string }).code).toBe('permissionDenied');
  });
});
