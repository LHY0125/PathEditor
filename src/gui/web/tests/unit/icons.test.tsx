import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import {
  PlusIcon,
  PencilIcon,
  TrashIcon,
  ArrowUpIcon,
  ArrowDownIcon,
  MoreIcon,
} from '@/components/ui/icons';

describe('图标', () => {
  it('全部渲染为 aria-hidden 的 svg', () => {
    for (const Icon of [PlusIcon, PencilIcon, TrashIcon, ArrowUpIcon, ArrowDownIcon, MoreIcon]) {
      const { container, unmount } = render(<Icon />);
      const svg = container.querySelector('svg');
      expect(svg).not.toBeNull();
      expect(svg?.getAttribute('aria-hidden')).toBe('true');
      unmount();
    }
  });

  it('接受 className 并透传', () => {
    const { container } = render(<PlusIcon className="custom-class" />);
    expect(container.querySelector('svg')?.classList.contains('custom-class')).toBe(true);
  });
});
