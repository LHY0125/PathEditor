import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { backend, parseProfileDiff } from '@/services/backend';

const mockInvoke = vi.mocked(invoke);

const emptyDiff = {
  system: { added: [], removed: [], reordered: [], toggled: [] },
  user: { added: [], removed: [], reordered: [], toggled: [] },
  envAdded: [],
  envRemoved: [],
  envModified: [],
};

const profileDiff = {
  left: 'A',
  right: 'B',
  leftDescription: null,
  rightDescription: 'B 描述',
  tagsAdded: ['x'],
  tagsRemoved: [],
  paths: emptyDiff,
};

const conflict = {
  id: 7,
  entityType: 'profile',
  entityUid: 'p-1',
  field: 'name',
  localValue: '"A"',
  remoteValue: '"B"',
  remoteDevice: 'dev-b',
  createdAt: 1,
  resolvedAt: null,
  resolution: null,
};

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('新增 IPC 包装', () => {
  it('profileDiff 走 profile_diff 并校验差异形状', async () => {
    mockInvoke.mockResolvedValue(profileDiff);
    const result = await backend.profileDiff('A', 'B');
    expect(mockInvoke).toHaveBeenCalledWith('profile_diff', { left: 'A', right: 'B' });
    expect(result.tagsAdded).toEqual(['x']);
    expect(result.paths.system.added).toEqual([]);
  });

  it('compareMany 走 compare_many 并逐项解析', async () => {
    mockInvoke.mockResolvedValue([{ baseUid: 'a', otherUid: 'b', diff: emptyDiff }]);
    const result = await backend.compareMany(['a', 'b']);
    expect(mockInvoke).toHaveBeenCalledWith('compare_many', { uids: ['a', 'b'] });
    expect(result).toHaveLength(1);
    expect(result[0].otherUid).toBe('b');
  });

  it('规则 CRUD 传递正确参数', async () => {
    mockInvoke.mockResolvedValue([]);
    await backend.rulesList();
    expect(mockInvoke).toHaveBeenLastCalledWith('rules_list');

    mockInvoke.mockResolvedValue('rule-1');
    await backend.rulesCreate({ name: '置顶 Java', contains: 'java', action: 'moveTop' });
    expect(mockInvoke).toHaveBeenLastCalledWith('rules_create', {
      request: { name: '置顶 Java', contains: 'java', action: 'moveTop' },
    });

    mockInvoke.mockResolvedValue(undefined);
    await backend.rulesSetEnabled('rule-1', false);
    expect(mockInvoke).toHaveBeenLastCalledWith('rules_set_enabled', {
      uid: 'rule-1',
      enabled: false,
    });

    await backend.rulesDelete('rule-1');
    expect(mockInvoke).toHaveBeenLastCalledWith('rules_delete', { uid: 'rule-1' });
  });

  it('rulesApply 解析规则结果', async () => {
    mockInvoke.mockResolvedValue({
      system: [{ path: 'C:\\Java', enabled: true }],
      user: [],
      hits: [{ ruleUid: 'r-1', matchedCount: 1 }],
    });
    const result = await backend.rulesApply([{ path: 'C:\\Java', enabled: true }], [], true);
    expect(mockInvoke).toHaveBeenCalledWith('rules_apply', {
      system: [{ path: 'C:\\Java', enabled: true }],
      user: [],
      record: true,
    });
    expect(result.hits[0].matchedCount).toBe(1);
  });

  it('保留策略读写与打开数据目录', async () => {
    mockInvoke.mockResolvedValue({
      snapshotKeepDays: 365,
      snapshotKeepMax: 500,
      eventKeepDays: 730,
    });
    const settings = await backend.retentionSettings();
    expect(mockInvoke).toHaveBeenLastCalledWith('retention_settings');
    expect(settings.snapshotKeepMax).toBe(500);

    mockInvoke.mockResolvedValue(undefined);
    await backend.retentionSave({
      snapshotKeepDays: 30,
      snapshotKeepMax: 100,
      eventKeepDays: 60,
    });
    expect(mockInvoke).toHaveBeenLastCalledWith('retention_save', {
      settings: { snapshotKeepDays: 30, snapshotKeepMax: 100, eventKeepDays: 60 },
    });

    await backend.openDataDir();
    expect(mockInvoke).toHaveBeenLastCalledWith('open_data_dir');
  });

  it('冲突详情与批量解决', async () => {
    mockInvoke.mockResolvedValue({ conflict, remoteContent: '{"name":"B"}' });
    const detail = await backend.syncConflictDetail(7);
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_conflict_detail', { id: 7 });
    expect(detail.remoteContent).toContain('B');

    mockInvoke.mockResolvedValue(undefined);
    await backend.syncResolveConflicts([7, 8], 'remote');
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_resolve_conflicts', {
      ids: [7, 8],
      resolution: 'remote',
    });
  });

  it('整份快照恢复要求对象返回值', async () => {
    mockInvoke.mockResolvedValue({});
    await backend.historyRestoreFullSnapshot('snap-1');
    expect(mockInvoke).toHaveBeenLastCalledWith('history_restore_full_snapshot', {
      uid: 'snap-1',
    });

    mockInvoke.mockResolvedValue(null);
    await expect(backend.historyRestoreFullSnapshot('snap-1')).rejects.toThrow(/无效契约/);
  });

  it('profile_diff 缺 paths 时拒绝', () => {
    expect(() => parseProfileDiff({ ...profileDiff, paths: undefined })).toThrow(/paths/);
  });
});

describe('同步与检索包装', () => {
  it('syncStatus / syncExport / syncImport / syncListConflicts', async () => {
    mockInvoke.mockResolvedValue({ pendingCount: 2, unresolvedConflicts: 1 });
    expect((await backend.syncStatus()).pendingCount).toBe(2);
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_status');

    mockInvoke.mockResolvedValue({ changeCount: 3, path: 'D:\\a.pesync' });
    await backend.syncExport('pw', 'D:\\a.pesync');
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_export', {
      passphrase: 'pw',
      path: 'D:\\a.pesync',
    });

    mockInvoke.mockResolvedValue({
      applied: 1,
      skipped: 0,
      conflicts: 0,
      skippedUnsupported: 0,
      renamedProfiles: 0,
    });
    await backend.syncImport('pw', 'D:\\a.pesync');
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_import', {
      passphrase: 'pw',
      path: 'D:\\a.pesync',
    });

    mockInvoke.mockResolvedValue([]);
    expect(await backend.syncListConflicts(true)).toEqual([]);
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_list_conflicts', {
      includeResolved: true,
    });
  });

  it('同步冲突单条解决不返回数据', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await backend.syncResolveConflict(1, 'local');
    expect(mockInvoke).toHaveBeenLastCalledWith('sync_resolve_conflict', {
      id: 1,
      resolution: 'local',
    });
  });

  it('compareSnapshots / search / refs 包装', async () => {
    mockInvoke.mockResolvedValue(emptyDiff);
    await backend.compareSnapshots('a', 'b');
    expect(mockInvoke).toHaveBeenLastCalledWith('compare_snapshots', {
      leftUid: 'a',
      rightUid: 'b',
    });

    mockInvoke.mockResolvedValue([]);
    expect(await backend.search('java', 10)).toEqual([]);
    expect(mockInvoke).toHaveBeenLastCalledWith('search', { query: 'java', limit: 10 });

    await backend.refsDependents('JAVA_HOME');
    expect(mockInvoke).toHaveBeenLastCalledWith('refs_dependents', { name: 'JAVA_HOME' });
    await backend.refsDependencies('JAVA_HOME');
    expect(mockInvoke).toHaveBeenLastCalledWith('refs_dependencies', { name: 'JAVA_HOME' });
    expect(await backend.refsCycles()).toEqual([]);
    expect(mockInvoke).toHaveBeenLastCalledWith('refs_cycles');
  });

  it('快照备注与固定列表', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await backend.historySetNote('s-1', '备注');
    expect(mockInvoke).toHaveBeenLastCalledWith('history_set_note', {
      uid: 's-1',
      note: '备注',
    });
    await backend.historySetPinned('s-1', true);
    expect(mockInvoke).toHaveBeenLastCalledWith('history_set_pinned', {
      uid: 's-1',
      pinned: true,
    });

    mockInvoke.mockResolvedValue([{ uid: 's-1', createdAt: 1, note: null }]);
    expect(await backend.historyListPinned()).toHaveLength(1);
  });
});

describe('快照读取包装', () => {
  const snapshot = {
    uid: 's-1',
    createdAt: 1,
    kind: 'current',
    system: [],
    user: [],
    env: [],
  };

  it('historyGetCurrent 接受 null 与快照对象', async () => {
    mockInvoke.mockResolvedValue(null);
    expect(await backend.historyGetCurrent()).toBeNull();

    mockInvoke.mockResolvedValue(snapshot);
    expect((await backend.historyGetCurrent())?.uid).toBe('s-1');
  });

  it('historyGetSnapshot 传递 reveal 并解析快照', async () => {
    mockInvoke.mockResolvedValue(snapshot);
    await backend.historyGetSnapshot('s-1', true);
    expect(mockInvoke).toHaveBeenLastCalledWith('history_get_snapshot', {
      uid: 's-1',
      reveal: true,
    });
    mockInvoke.mockResolvedValue({ ...snapshot, uid: 7 });
    await expect(backend.historyGetSnapshot('s-1')).rejects.toThrow(/uid/);
  });

  it('historyRevealEnvValue 只接受字符串', async () => {
    mockInvoke.mockResolvedValue('secret');
    await expect(backend.historyRevealEnvValue('s-1', 'user', 'TOKEN')).resolves.toBe('secret');
    expect(mockInvoke).toHaveBeenLastCalledWith('history_reveal_env_value', {
      uid: 's-1',
      hive: 'user',
      name: 'TOKEN',
    });
    mockInvoke.mockResolvedValue(42);
    await expect(backend.historyRevealEnvValue('s-1', 'user', 'TOKEN')).rejects.toThrow(/字符串/);
  });
});

it('statsDashboard 解析统计面板契约', async () => {
  mockInvoke.mockResolvedValue({
    days: [{ day: '2026-11-09', pathItems: 1, envItems: 2, events: 3, conflicts: 0 }],
    topTargets: [{ target: 'Path', count: 2 }],
    profileApplies: [{ name: 'Work', applyCount: 1, lastAppliedAt: null }],
    currentAvgLen: 12.5,
    currentPathItems: 8,
    scanRuns: 2,
    conflictRate: 0.1,
    topConflicts: [{ target: 'Path', count: 1 }],
  });
  const dashboard = await backend.statsDashboard(7);
  expect(mockInvoke).toHaveBeenLastCalledWith('stats_dashboard', { days: 7 });
  expect(dashboard.days[0].events).toBe(3);
  expect(dashboard.topConflicts[0].count).toBe(1);
});

it('dbHealth 解析健康快照', async () => {
  mockInvoke.mockResolvedValue({
    path: 'C:\\db.sqlite',
    fileBytes: 1024,
    walBytes: 0,
    shmBytes: 0,
    schemaVersion: 6,
    expectedSchemaVersion: 6,
    integrity: 'ok',
    integrityOk: true,
    tableRows: [{ name: 'profiles', rows: 2 }],
    lastPruneAt: null,
  });
  const health = await backend.dbHealth();
  expect(mockInvoke).toHaveBeenLastCalledWith('db_health');
  expect(health.schemaVersion).toBe(6);
  expect(health.tableRows[0].rows).toBe(2);
});

it('profileRename 只接受布尔返回值', async () => {
  mockInvoke.mockResolvedValue(true);
  await expect(backend.profileRename('A', 'B')).resolves.toBe(true);
  expect(mockInvoke).toHaveBeenLastCalledWith('profile_rename', { oldName: 'A', newName: 'B' });
});
