import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';

// 面板只经 controller 读状态、经回调触发 IPC —— 这里直接构造 controller，
// 不 mock backend（面板自身不 import backend，这正是分层要求的体现）。
//
// 为什么必须有这个文件：e2e 不在 `npm run verify` 内（无 CI 跑它），
// 而删除项名列表是验收标准 12 的明面要求、也是整条链路最不可逆的部分。
// 只靠 e2e 等于该渲染在门控里没有任何保护。
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

import { EnvBackupPanel } from '@/components/dialogs/env-backup/EnvBackupPanel';
import type { EnvBackupController } from '@/components/dialogs/env-backup/use-env-backup';
import type { EnvBackupInfo, RestorePreview } from '@/core/env-backup';

const info: EnvBackupInfo = {
  file: 'env_backup_20260922_120000_000.json',
  path: 'C:\\b\\env_backup_20260922_120000_000.json',
  timestamp: '20260922_120000_000',
  sizeBytes: 2048,
  variableCount: 0,
};

/** 含两条删除项的差异；四个计数两两不等，避免渲染串位时测不出。 */
const previewWithRemovals: RestorePreview = {
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

/** 无删除项的差异：删除警告区块不得出现（成对反例）。 */
const previewWithoutRemovals: RestorePreview = {
  changes: [{ hive: 'user', name: 'NEW_ONLY', kind: 'added' }],
  added: 1,
  modified: 0,
  removed: 0,
  conflicts: 0,
};

/** 只喂面板消费的字段；回调全部为 no-op（面板不发起 IPC）。 */
function controller(over: Partial<EnvBackupController> = {}): EnvBackupController {
  return {
    backups: [info],
    busy: false,
    creating: false,
    selected: info,
    preview: null,
    summary: null,
    outcome: null,
    createdPath: null,
    error: null,
    createBackup: vi.fn().mockResolvedValue(undefined),
    select: vi.fn().mockResolvedValue(undefined),
    restore: vi.fn().mockResolvedValue(undefined),
    ...over,
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('EnvBackupPanel 删除项渲染（验收标准 12）', () => {
  it('removed > 0 时渲染删除警告区块并逐个列出变量名（带 hive 前缀）', () => {
    render(
      <EnvBackupPanel
        controller={controller({
          preview: previewWithRemovals,
          summary: {
            hasChanges: true,
            hasConflicts: true,
            removedNames: ['user:OLD_VAR', 'system:LEGACY_HOME'],
            text: '新增 1、删除 2、冲突 3',
          },
        })}
      />,
    );

    const block = screen.getByTestId('backup-removed-names');
    expect(block).toBeTruthy();
    // 必须是**变量名本身**，而不是只有计数
    expect(block.textContent).toContain('user:OLD_VAR');
    expect(block.textContent).toContain('system:LEGACY_HOME');
  });

  it('removed === 0 时不得渲染删除警告区块（成对反例）', () => {
    render(
      <EnvBackupPanel
        controller={controller({
          preview: previewWithoutRemovals,
          summary: {
            hasChanges: true,
            hasConflicts: false,
            removedNames: [],
            text: '新增 1',
          },
        })}
      />,
    );

    expect(screen.queryByTestId('backup-removed-names')).toBeNull();
  });

  it('四个计数按各自字段渲染，不串位', () => {
    render(
      <EnvBackupPanel
        controller={controller({
          preview: previewWithRemovals,
          summary: {
            hasChanges: true,
            hasConflicts: true,
            removedNames: [],
            text: '摘要',
          },
        })}
      />,
    );

    // 四个标签各自后面跟自己的数字：1 / 0 / 2 / 3 两两不等
    expect(screen.getByText('新增').nextElementSibling?.textContent).toBe('1');
    expect(screen.getByText('修改').nextElementSibling?.textContent).toBe('0');
    expect(screen.getByText('删除').nextElementSibling?.textContent).toBe('2');
    expect(screen.getByText('冲突').nextElementSibling?.textContent).toBe('3');
  });

  it('恢复结果含失败项时逐条展示（不吞 failures）', () => {
    render(
      <EnvBackupPanel
        controller={controller({
          preview: previewWithRemovals,
          summary: {
            hasChanges: true,
            hasConflicts: false,
            removedNames: [],
            text: '摘要',
          },
          outcome: { applied: 1, skipped: 0, failures: ['[系统] LEGACY_HOME: 拒绝访问'] },
        })}
      />,
    );

    const outcome = screen.getByTestId('backup-outcome');
    expect(outcome.textContent).toContain('[系统] LEGACY_HOME: 拒绝访问');
    expect(outcome.textContent).toContain('失败 1 项');
  });

  it('busy 时恢复按钮禁用（避免并发恢复）', () => {
    render(
      <EnvBackupPanel
        controller={controller({
          busy: true,
          preview: previewWithRemovals,
          summary: {
            hasChanges: true,
            hasConflicts: false,
            removedNames: [],
            text: '摘要',
          },
        })}
      />,
    );

    expect(
      (screen.getByRole('button', { name: '正在恢复...' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
