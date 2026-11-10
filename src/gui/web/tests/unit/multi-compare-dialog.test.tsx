import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import i18n from '@/i18n';
import { MultiCompareDialog } from '@/components/history/MultiCompareDialog';

/**
 * N 路对比不能只给计数：逐条路径、重排与启停变化都要可见，
 * 否则用户无法判断差异是否值得应用。
 */

beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

afterEach(() => {
  cleanup();
});

const compareManyMock = vi.fn();

vi.mock('@/services/backend', () => ({
  backend: {
    compareMany: (...a: unknown[]) => compareManyMock(...a),
  },
}));

const emptyDiff = { added: [], removed: [], reordered: [], toggled: [] };

describe('MultiCompareDialog', () => {
  it('渲染逐条 added / reordered / toggled 明细', async () => {
    compareManyMock.mockReset();
    compareManyMock.mockResolvedValue([
      {
        baseUid: 'a',
        otherUid: 'b',
        diff: {
          system: {
            ...emptyDiff,
            added: [{ path: 'C:\\NEW', enabled: true }],
            reordered: [{ path: 'C:\\MOVED', leftIndex: 0, rightIndex: 2 }],
            toggled: [{ path: 'C:\\TOGGLED', leftEnabled: true, rightEnabled: false }],
          },
          user: emptyDiff,
          envAdded: [],
          envRemoved: [],
          envModified: [],
          envDetails: [],
        },
      },
    ]);

    render(<MultiCompareDialog uids={['a', 'b']} onClose={() => {}} />);

    await waitFor(() => expect(screen.getByText(/C:\\NEW/)).toBeTruthy());
    expect(screen.getByText(/C:\\MOVED/)).toBeTruthy();
    expect(screen.getByText(/C:\\TOGGLED/)).toBeTruthy();
    expect(screen.getByText(/第 1 位 → 第 3 位/)).toBeTruthy();
  });
});
