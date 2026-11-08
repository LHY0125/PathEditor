import { describe, it, expect } from 'vitest';
import {
  parseCompareResult,
  parseCycleList,
  parseDashboard,
  parseDbHealth,
  parseRefEdges,
  parseScanRunDiff,
  parseScanRuns,
  parseSearchHits,
} from '@/services/backend';

/**
 * 检索 / 统计 / 对比 / 引用图的 IPC 形状校验。
 *
 * 每个 `parseXxx` 覆盖四类输入：正常 / 缺字段 / 类型错 / 非法值。
 * 这类校验的价值不在于「测试通过」，而在于**后端形状漂移时前端会立刻报错**，
 * 而不是把 `undefined` 渲染成界面上的空白。
 */

describe('parseSearchHits', () => {
  it('接受合法响应', () => {
    const hits = parseSearchHits([
      {
        kind: 'path',
        refUid: 'snap#system',
        content: 'C:\\Java\\jdk21\\bin',
        snippet: '[jdk21]',
        rank: -1.5,
      },
    ]);
    expect(hits).toHaveLength(1);
    expect(hits[0].refUid).toBe('snap#system');
    expect(hits[0].content).toBe('C:\\Java\\jdk21\\bin');
  });

  it('拒绝非数组', () => {
    expect(() => parseSearchHits({})).toThrow(/必须返回数组/);
  });

  it('拒绝缺字段的条目', () => {
    expect(() => parseSearchHits([{ kind: 'path' }])).toThrow(/形状无效/);
  });

  it('拒绝 rank 为字符串的条目', () => {
    expect(() =>
      parseSearchHits([{ kind: 'path', refUid: 'x', snippet: 's', rank: '-1' }]),
    ).toThrow(/形状无效/);
  });
});

describe('parseDashboard', () => {
  const valid = {
    days: [{ day: '2026-10-21', pathItems: 2, envItems: 1, events: 3, conflicts: 0 }],
    topTargets: [{ target: 'Path', count: 3 }],
    profileApplies: [{ name: 'Work', applyCount: 2, lastAppliedAt: 1234 }],
    currentAvgLen: 4.5,
    currentPathItems: 2,
  };

  it('接受合法响应', () => {
    const d = parseDashboard(valid);
    expect(d.days[0].events).toBe(3);
    expect(d.currentAvgLen).toBe(4.5);
  });

  it('接受 lastAppliedAt 为 null（从未应用）', () => {
    const d = parseDashboard({
      ...valid,
      profileApplies: [{ name: 'Work', applyCount: 0, lastAppliedAt: null }],
    });
    expect(d.profileApplies[0].lastAppliedAt).toBeNull();
  });

  it('拒绝缺 days', () => {
    expect(() => parseDashboard({ ...valid, days: undefined })).toThrow(/days 必须是数组/);
  });

  it('拒绝缺 current* 字段', () => {
    expect(() => parseDashboard({ ...valid, currentAvgLen: undefined })).toThrow(
      /current\* 字段必须是数字/,
    );
  });

  it('拒绝 days 条目里 events 为字符串', () => {
    expect(() =>
      parseDashboard({
        ...valid,
        days: [{ day: 'x', pathItems: 1, envItems: 1, events: '3', conflicts: 0 }],
      }),
    ).toThrow(/days 条目形状无效/);
  });

  it('缺冲突统计字段时按零值兼容', () => {
    const d = parseDashboard(valid);
    expect(d.scanRuns).toBe(0);
    expect(d.conflictRate).toBe(0);
    expect(d.topConflicts).toEqual([]);
  });

  it('解析冲突统计与 topConflicts', () => {
    const d = parseDashboard({
      ...valid,
      scanRuns: 4,
      conflictRate: 0.25,
      topConflicts: [{ target: 'a.exe', count: 2 }],
    });
    expect(d.scanRuns).toBe(4);
    expect(d.topConflicts[0].target).toBe('a.exe');
  });

  it('拒绝非法的 topConflicts 条目', () => {
    expect(() => parseDashboard({ ...valid, topConflicts: [{ target: 1 }] })).toThrow(
      /topConflicts/,
    );
  });
});

describe('parseCompareResult', () => {
  const diff = { added: [], removed: [], reordered: [], toggled: [] };
  const valid = {
    system: diff,
    user: diff,
    envAdded: [],
    envRemoved: [],
    envModified: [],
  };

  it('接受双 hive 均为空的响应', () => {
    const r = parseCompareResult(valid);
    expect(r.system.added).toHaveLength(0);
    expect(r.user.added).toHaveLength(0);
  });

  // 守卫：后端退回「只返回 system」的形状时，前端必须拒绝，
  // 而不是把 user 当成空差异静默显示「用户 PATH 没变化」
  it('缺 user 侧必须报错', () => {
    expect(() => parseCompareResult({ ...valid, user: undefined })).toThrow(
      /必须同时返回 system 与 user 两侧/,
    );
  });

  it('缺 system 侧必须报错', () => {
    expect(() => parseCompareResult({ ...valid, system: undefined })).toThrow(
      /必须同时返回 system 与 user 两侧/,
    );
  });

  it('拒绝 toggled 缺字段（启用状态是跨层契约）', () => {
    expect(() =>
      parseCompareResult({
        ...valid,
        system: { added: [], removed: [], reordered: [] },
      }),
    ).toThrow(/toggled 必须是数组/);
  });

  it('拒绝 env_* 非字符串数组', () => {
    expect(() => parseCompareResult({ ...valid, envAdded: [1] })).toThrow(
      /envAdded 必须是字符串数组/,
    );
  });

  it('缺 envDetails 时按空数组兼容', () => {
    expect(parseCompareResult(valid).envDetails).toEqual([]);
  });

  it('解析 envDetails 的 hive/name/change', () => {
    const r = parseCompareResult({
      ...valid,
      envDetails: [{ hive: 'user', name: 'JAVA_HOME', change: 'modified' }],
    });
    expect(r.envDetails).toEqual([{ hive: 'user', name: 'JAVA_HOME', change: 'modified' }]);
  });

  it('拒绝非法的 change 取值', () => {
    expect(() =>
      parseCompareResult({
        ...valid,
        envDetails: [{ hive: 'user', name: 'X', change: 'bogus' }],
      }),
    ).toThrow(/envDetails/);
  });
});

describe('parseRefEdges', () => {
  it('接受合法边', () => {
    const edges = parseRefEdges([
      { fromHive: 'system', fromName: 'PATH', toName: 'JAVA_HOME', depth: 1 },
    ]);
    expect(edges[0].toName).toBe('JAVA_HOME');
  });

  it('拒绝缺 depth', () => {
    expect(() => parseRefEdges([{ fromHive: 'system', fromName: 'PATH', toName: 'X' }])).toThrow(
      /形状无效/,
    );
  });
});

describe('parseCycleList', () => {
  it('接受空环列表', () => {
    expect(parseCycleList([])).toEqual([]);
  });

  it('接受一个环', () => {
    expect(parseCycleList([['JAVA_HOME', 'JDK_HOME']])).toEqual([['JAVA_HOME', 'JDK_HOME']]);
  });

  it('拒绝元素不是字符串数组', () => {
    expect(() => parseCycleList([['A', 1]])).toThrow(/每个环必须是字符串数组/);
  });
});

describe('parseScanRuns', () => {
  it('接受合法扫描列表', () => {
    const runs = parseScanRuns([{ uid: 'r1', createdAt: 1, conflicts: ['a.exe'], toolCount: 3 }]);
    expect(runs[0].conflicts).toEqual(['a.exe']);
  });

  it('拒绝缺 uid 的条目', () => {
    expect(() => parseScanRuns([{ createdAt: 1, conflicts: [], toolCount: 0 }])).toThrow(
      /ScanRunView/,
    );
  });

  it('拒绝 conflicts 非字符串数组', () => {
    expect(() =>
      parseScanRuns([{ uid: 'r1', createdAt: 1, conflicts: [1], toolCount: 0 }]),
    ).toThrow(/字符串数组/);
  });
});

describe('parseScanRunDiff', () => {
  it('接受合法差异', () => {
    const diff = parseScanRunDiff({
      appeared: ['a.exe'],
      disappeared: [],
      persistent: ['b.exe'],
      lastSeen: [{ conflict: 'a.exe', runUid: 'r2', seenAt: 2 }],
      firstSeen: [{ conflict: 'a.exe', runUid: 'r2', seenAt: 2 }],
    });
    expect(diff.appeared).toEqual(['a.exe']);
    expect(diff.lastSeen[0].runUid).toBe('r2');
  });

  it('拒绝缺 appeared 的响应', () => {
    expect(() =>
      parseScanRunDiff({ disappeared: [], persistent: [], lastSeen: [], firstSeen: [] }),
    ).toThrow(/appeared/);
  });
});

describe('parseDbHealth', () => {
  const base = {
    path: 'C:/Users/x/.patheditor/patheditor.db',
    fileBytes: 1024,
    walBytes: 0,
    shmBytes: 0,
    schemaVersion: 5,
    expectedSchemaVersion: 5,
    integrity: 'ok',
    integrityOk: true,
    tableRows: [{ name: 'snapshots', rows: 3 }],
    lastPruneAt: null,
  };

  it('接受最小健康契约', () => {
    const h = parseDbHealth(base);
    expect(h.integrityOk).toBe(true);
    expect(h.tableRows[0].name).toBe('snapshots');
  });

  it('拒绝 tableRows 形状错误的响应', () => {
    expect(() => parseDbHealth({ ...base, tableRows: [{ name: 1, rows: 2 }] })).toThrow(
      /tableRows/,
    );
  });

  it('拒绝 lastPruneAt 既非数字也非 null', () => {
    expect(() => parseDbHealth({ ...base, lastPruneAt: 'yesterday' })).toThrow(/形状/);
  });
});
