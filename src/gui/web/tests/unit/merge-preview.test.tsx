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
      userPaths: [{ path: 'D:\\UserApp', enabled: true }],
      searchQuery: '',
    };
    return selector(state);
  }),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: (options: Record<string, number>) => ({
    getVirtualItems: () => {
      // return an array of objects to mock virtual items
      return Array.from({ length: options.count }).map((_, index) => ({
        index,
        start: index * 28,
        size: 28,
        key: `mock-key-${index}`,
      }));
    },
    getTotalSize: () => options.count * 28,
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

  // 回归：`.path-row` 是 `position: absolute; top: 0`，虚拟列表必须给每行
  // 写 height + translateY。漏掉时所有行叠在 y=0，渲染成一片重影。
  // jsdom 不算 CSS 布局，所以这里钉的是「组件是否把虚拟偏移写进内联样式」。
  it('每行按虚拟滚动偏移定位，不得全部叠在 y=0', () => {
    const { container } = render(<MergePreview />);
    const rows = container.querySelectorAll<HTMLElement>('.path-row');

    expect(rows.length).toBe(3);
    rows.forEach((row, index) => {
      expect(row.style.height).toBe('28px');
      expect(row.style.transform).toBe(`translateY(${index * 28}px)`);
    });
  });
});
