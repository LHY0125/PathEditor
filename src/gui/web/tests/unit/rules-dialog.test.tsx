import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup, fireEvent } from '@testing-library/react';
import i18n from '@/i18n';
import { RulesDialog } from '@/components/dialogs/RulesDialog';

/**
 * backend 的 rejection 是普通对象 `{code, message}`，不是 Error。
 * 直接 `String(err)` 会让用户看到 `[object Object]`；必须走 errorMessage()。
 */

beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

afterEach(() => {
  cleanup();
});

const rulesListMock = vi.fn();
const rulesCreateMock = vi.fn();

vi.mock('@/services/backend', () => ({
  backend: {
    rulesList: (...a: unknown[]) => rulesListMock(...a),
    rulesCreate: (...a: unknown[]) => rulesCreateMock(...a),
  },
}));

describe('RulesDialog', () => {
  it('结构化 rejection 显示 message，而不是 [object Object]', async () => {
    rulesListMock.mockReset();
    rulesCreateMock.mockReset();
    rulesListMock.mockResolvedValue([]);
    rulesCreateMock.mockRejectedValue({ code: 'invalidValue', message: '规则名称过长' });

    render(<RulesDialog open onClose={() => {}} />);
    await waitFor(() => expect(rulesListMock).toHaveBeenCalled());

    fireEvent.change(screen.getByLabelText('规则名'), { target: { value: 'work' } });
    fireEvent.change(screen.getByLabelText('匹配子串'), { target: { value: 'C:\\' } });
    fireEvent.click(screen.getByRole('button', { name: '添加规则' }));

    await waitFor(() => expect(screen.getByText('规则名称过长')).toBeTruthy());
    expect(screen.queryByText('[object Object]')).toBeNull();
  });
});
