import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup, fireEvent } from '@testing-library/react';
import i18n from '@/i18n';
import { DegradedBanner } from '@/components/layout/DegradedBanner';

/**
 * 降级条幅必须把根因和可执行动作带出来。
 *
 * 旧实现只显示通用文案「重启后可重试」；对 schema 不兼容/旧加密库，
 * 重启永远不会成功，用户只能看到历史功能消失却无法自助恢复。
 */

beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

afterEach(() => {
  cleanup();
});

const historyGetCurrentMock = vi.fn();
const openDataDirMock = vi.fn();

vi.mock('@/services/backend', () => ({
  backend: {
    historyGetCurrent: (...a: unknown[]) => historyGetCurrentMock(...a),
    openDataDir: (...a: unknown[]) => openDataDirMock(...a),
  },
}));

describe('DegradedBanner', () => {
  it('显示数据库根因并允许打开数据目录', async () => {
    historyGetCurrentMock.mockReset();
    openDataDirMock.mockReset();
    historyGetCurrentMock.mockRejectedValue({
      code: 'migration',
      message: '数据库结构版本 6 不受支持（当前 7）',
    });
    openDataDirMock.mockResolvedValue(undefined);

    render(<DegradedBanner />);

    await waitFor(() => expect(screen.getByText(/数据库结构版本 6 不受支持/)).toBeTruthy());
    expect(screen.queryByText(/重启后可重试/)).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '打开数据目录' }));
    await waitFor(() => expect(openDataDirMock).toHaveBeenCalledTimes(1));
  });

  it('普通 database 错误不提示删除旧库', async () => {
    historyGetCurrentMock.mockReset();
    historyGetCurrentMock.mockRejectedValue({
      code: 'database',
      message: 'database is locked',
    });

    render(<DegradedBanner />);
    await waitFor(() => expect(screen.getByText(/database is locked/)).toBeTruthy());
    expect(screen.getByText(/不要直接删除数据库/)).toBeTruthy();
    expect(screen.queryByText(/备份后删除旧库/)).toBeNull();
  });
});
