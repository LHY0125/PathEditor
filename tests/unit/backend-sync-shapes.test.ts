import { describe, it, expect } from 'vitest';
import {
  parseSyncConflicts,
  parseSyncExportReport,
  parseSyncImportReport,
  parseSyncStatus,
} from '@/services/backend';

/**
 * M3 同步 IPC 的形状校验（Task 9）。
 *
 * 每个 `parseXxx` 覆盖四类输入：正常 / 缺字段 / 类型错 / 非法值。
 * 与 M2 的 `backend-m2-shapes.test.ts` 同一口径：校验的价值在于
 * **后端形状漂移时前端立刻报错**，而不是把 `undefined` 渲染成空白。
 */

describe('parseSyncExportReport', () => {
  it('接受合法响应', () => {
    const r = parseSyncExportReport({ changeCount: 3, path: 'D:\\a.pesync' });
    expect(r.changeCount).toBe(3);
    expect(r.path).toBe('D:\\a.pesync');
  });

  it('拒绝缺字段', () => {
    expect(() => parseSyncExportReport({ changeCount: 3 })).toThrow(/无效的导出结果/);
    expect(() => parseSyncExportReport({ path: 'x' })).toThrow(/无效的导出结果/);
  });

  it('拒绝类型错误', () => {
    expect(() => parseSyncExportReport({ changeCount: '3', path: 'x' })).toThrow(/无效/);
    expect(() => parseSyncExportReport({ changeCount: 3, path: 7 })).toThrow(/无效/);
  });

  it('拒绝非对象', () => {
    expect(() => parseSyncExportReport(null)).toThrow(/无效的导出结果/);
    expect(() => parseSyncExportReport([])).toThrow(/无效的导出结果/);
  });
});

describe('parseSyncImportReport', () => {
  it('接受合法响应（含全 0）', () => {
    expect(parseSyncImportReport({ applied: 1, skipped: 2, conflicts: 3 })).toEqual({
      applied: 1,
      skipped: 2,
      conflicts: 3,
    });
    expect(parseSyncImportReport({ applied: 0, skipped: 0, conflicts: 0 }).applied).toBe(0);
  });

  it('拒绝缺字段', () => {
    expect(() => parseSyncImportReport({ applied: 1, skipped: 2 })).toThrow(/无效的导入结果/);
  });

  it('拒绝类型错误', () => {
    expect(() => parseSyncImportReport({ applied: '1', skipped: 0, conflicts: 0 })).toThrow(/无效/);
  });
});

describe('parseSyncStatus', () => {
  it('接受合法响应', () => {
    expect(parseSyncStatus({ pendingCount: 5, unresolvedConflicts: 1 })).toEqual({
      pendingCount: 5,
      unresolvedConflicts: 1,
    });
  });

  it('拒绝缺字段与类型错', () => {
    expect(() => parseSyncStatus({ pendingCount: 5 })).toThrow(/无效的同步状态/);
    expect(() => parseSyncStatus({ pendingCount: '5', unresolvedConflicts: 0 })).toThrow(/无效/);
  });
});

describe('parseSyncConflicts', () => {
  const valid = {
    id: 1,
    entityType: 'profile',
    entityUid: 'p-1',
    field: 'name',
    localValue: '"A"',
    remoteValue: '"B"',
    remoteDevice: 'dev-b',
    createdAt: 100,
    resolvedAt: null,
    resolution: null,
  };

  it('接受合法响应（含 null 字段与已解决项）', () => {
    const list = parseSyncConflicts([
      valid,
      { ...valid, id: 2, field: null, resolvedAt: 200, resolution: 'remote' },
    ]);
    expect(list).toHaveLength(2);
    expect(list[0].field).toBe('name');
    expect(list[1].field).toBeNull();
    expect(list[1].resolution).toBe('remote');
  });

  it('拒绝非数组', () => {
    expect(() => parseSyncConflicts({})).toThrow(/必须返回数组/);
  });

  it('拒绝缺字段', () => {
    const missing = {
      id: 1,
      entityType: 'profile',
      entityUid: 'p-1',
      field: 'name',
      localValue: null,
      remoteValue: null,
      createdAt: 100,
      resolvedAt: null,
      resolution: null,
    };
    expect(() => parseSyncConflicts([missing])).toThrow(/形状无效/);
  });

  it('拒绝把 null 写成 undefined 以外的类型', () => {
    expect(() => parseSyncConflicts([{ ...valid, field: 42 }])).toThrow(/形状无效/);
    expect(() => parseSyncConflicts([{ ...valid, resolvedAt: '200' }])).toThrow(/形状无效/);
  });

  it('接收空数组（无冲突是正常态）', () => {
    expect(parseSyncConflicts([])).toEqual([]);
  });
});
