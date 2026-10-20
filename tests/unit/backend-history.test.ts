import { describe, it, expect } from 'vitest';
import { parseEventPage, parseSnapshotView } from '@/services/backend';

describe('parseEventPage', () => {
  it('拒绝缺少 items 的响应', () => {
    expect(() => parseEventPage({ total: 1 })).toThrow(/无效/);
  });

  it('拒绝 items 中 action 非法的项', () => {
    expect(() =>
      parseEventPage({ items: [{ uid: 'a', createdAt: 1, source: 'gui', action: 123 }], total: 1 }),
    ).toThrow(/action/);
  });

  it('拒绝 result 非法的项', () => {
    expect(() =>
      parseEventPage({
        items: [{ uid: 'a', createdAt: 1, source: 'gui', action: 'save', result: 'nope' }],
        total: 1,
      }),
    ).toThrow(/result/);
  });

  it('接受合法响应并保留 total 语义', () => {
    const page = parseEventPage({
      items: [
        {
          uid: 'a',
          createdAt: 1,
          source: 'gui',
          action: 'save',
          hive: 'user',
          target: 'Path',
          result: 'success',
        },
      ],
      total: 42,
    });
    expect(page.items).toHaveLength(1);
    expect(page.items[0].action).toBe('save');
    expect(page.items[0].hive).toBe('user');
    // total 是总行数，不是本页条数
    expect(page.total).toBe(42);
  });
});

describe('parseSnapshotView', () => {
  const base = {
    uid: 's1',
    createdAt: 1,
    kind: 'current',
    system: [{ path: 'C:\\Windows', enabled: true }],
    user: [],
    env: [],
  };

  it('拒绝缺少 uid 的快照', () => {
    expect(() => parseSnapshotView({ ...base, uid: undefined }, 'x')).toThrow(/uid/);
  });

  it('接受 value 为 null 的敏感行（Rust 侧掩码）', () => {
    const view = parseSnapshotView(
      {
        ...base,
        env: [
          {
            hive: 'user',
            name: 'MY_TOKEN',
            kind: 'String',
            value: null,
            revision: 'r',
            sensitive: true,
            restorable: true,
          },
        ],
      },
      'x',
    );
    expect(view.env[0].value).toBeNull();
    expect(view.env[0].sensitive).toBe(true);
  });

  it('拒绝 env 值类型错误', () => {
    expect(() =>
      parseSnapshotView(
        {
          ...base,
          env: [
            {
              hive: 'user',
              name: 'A',
              kind: 'String',
              value: 42,
              revision: 'r',
              sensitive: false,
              restorable: true,
            },
          ],
        },
        'x',
      ),
    ).toThrow(/值/);
  });
});
