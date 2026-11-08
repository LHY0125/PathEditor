import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import i18n from '@/i18n';

vi.mock('@/services/backend', () => ({
  backend: {
    historySetNote: vi.fn(async () => undefined),
    historySetPinned: vi.fn(async () => undefined),
    historyRevealEnvValue: vi.fn(),
  },
}));

import { backend, type SnapshotView } from '@/services/backend';
import { SnapshotDetail } from '@/components/history/SnapshotDetail';

beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

function snapshot(overrides: Partial<SnapshotView> = {}): SnapshotView {
  return {
    uid: 'snap-1',
    createdAt: 1,
    kind: 'history',
    system: [],
    user: [],
    env: [],
    note: null,
    pinned: false,
    ...overrides,
  };
}

describe('SnapshotDetail 备注与固定', () => {
  beforeEach(() => vi.clearAllMocks());

  it('保存备注时传裁剪后的文本；全空白按清空处理', async () => {
    render(<SnapshotDetail snapshot={snapshot()} />);

    const input = screen.getByPlaceholderText('快照备注…');
    fireEvent.change(input, { target: { value: '  装驱动前  ' } });
    fireEvent.click(screen.getByRole('button', { name: '保存备注' }));
    await waitFor(() => expect(backend.historySetNote).toHaveBeenCalledWith('snap-1', '装驱动前'));

    fireEvent.change(input, { target: { value: '   ' } });
    fireEvent.click(screen.getByRole('button', { name: '保存备注' }));
    await waitFor(() => expect(backend.historySetNote).toHaveBeenLastCalledWith('snap-1', null));
  });

  it('固定按钮按当前状态取反', async () => {
    const { rerender } = render(<SnapshotDetail snapshot={snapshot()} />);
    fireEvent.click(screen.getByRole('button', { name: '固定快照' }));
    await waitFor(() => expect(backend.historySetPinned).toHaveBeenCalledWith('snap-1', true));

    rerender(<SnapshotDetail snapshot={snapshot({ pinned: true })} />);
    fireEvent.click(screen.getByRole('button', { name: '取消固定' }));
    await waitFor(() => expect(backend.historySetPinned).toHaveBeenLastCalledWith('snap-1', false));
  });
});
