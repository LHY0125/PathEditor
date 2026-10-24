import { describe, it, expect } from 'vitest';
import {
  parseCompareResult,
  parseCycleList,
  parseDashboard,
  parseRefEdges,
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
