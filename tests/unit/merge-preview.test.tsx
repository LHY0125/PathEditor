// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { MergePreview } from '../../src/components/path-list/MergePreview';

vi.mock('@/store/app-store', () => ({
  useAppStore: vi.fn((selector) => {
    const state = {
      sysPaths: [
        { path: 'C:\\Windows', enabled: true },
        { path: 'C:\\Disabled', enabled: false },
      ],
      userPaths: [
        { path: 'D:\\UserApp', enabled: true },
      ],
      searchQuery: '',
    };
    return selector(state);
  }),
}));

vi.mock('@/i18n', () => ({
  default: { t: vi.fn((key: string) => key) },
}));

describe('MergePreview', () => {
  it('合并显示系统+用户路径', () => {
    const { container } = render(<MergePreview />);
    const text = container.textContent || '';
    expect(text).toContain('C:\\Windows');
    expect(text).toContain('D:\\UserApp');
  });

  it('disabled 路径在表格中存在', () => {
    const { container } = render(<MergePreview />);
    const text = container.textContent || '';
    expect(text).toContain('C:\\Disabled');
  });
});
