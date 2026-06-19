// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { AnalyzeDialog } from '../../src/components/dialogs/AnalyzeDialog';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === 'scan_conflicts') return Promise.resolve([]);
    if (cmd === 'scan_tools') return Promise.resolve([]);
    return Promise.resolve(undefined);
  }),
}));

vi.mock('@/store/app-store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector) => {
      const state = { sysPaths: [], userPaths: [] };
      return selector(state);
    }),
    { getState: () => ({ sysPaths: [], userPaths: [] }) },
  ),
}));

vi.mock('@/i18n', () => ({
  default: { t: vi.fn((key: string) => key) },
}));

describe('AnalyzeDialog', () => {
  it('渲染冲突检测和工具清单标签页，不崩溃', () => {
    const { container } = render(<AnalyzeDialog open={true} onClose={() => {}} />);
    const text = container.textContent || '';
    expect(text).toContain('analyze.conflicts');
    expect(text).toContain('analyze.tools');
  });
});
