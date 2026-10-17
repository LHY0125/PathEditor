import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { Menu, type MenuGroupDef } from '@/components/ui/Menu';

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效（同 env-var-toolbar.test.tsx / button.test.tsx）。
afterEach(() => {
  cleanup();
});

function makeGroups(onSelect = vi.fn()): MenuGroupDef[] {
  return [
    { key: 'maint', label: '维护', items: [{ key: 'clean', label: '清理无效路径', onSelect }] },
    {
      key: 'app',
      label: '应用',
      items: [{ key: 'help', label: '帮助', onSelect, shortcut: 'F1' }],
    },
  ];
}

describe('Menu', () => {
  it('初始关闭：不渲染菜单面板', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击触发按钮打开菜单', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    expect(screen.getByTestId('menu-panel')).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /清理无效路径/ })).toBeInTheDocument();
  });

  it('再次点击触发按钮关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    fireEvent.click(trigger);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('按 Escape 关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击外部关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击条目触发回调并关闭', () => {
    const onSelect = vi.fn();
    render(<Menu groups={makeGroups(onSelect)} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: /清理无效路径/ }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('显示快捷键提示', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    expect(screen.getByText('F1')).toBeInTheDocument();
  });

  it('disabled 条目不可点击', () => {
    const onSelect = vi.fn();
    const groups: MenuGroupDef[] = [
      { key: 'g', items: [{ key: 'a', label: '禁用项', onSelect, disabled: true }] },
    ];
    render(<Menu groups={groups} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: '禁用项' }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('footer 条目渲染在分隔线之后', () => {
    const onSelect = vi.fn();
    render(
      <Menu
        groups={makeGroups()}
        triggerAriaLabel="更多操作"
        footer={{ key: 'cancel', label: '取消并关闭', onSelect }}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: '取消并关闭' }));
    expect(onSelect).toHaveBeenCalled();
  });

  it('关闭后焦点回到触发按钮', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(document.activeElement).toBe(trigger);
  });

  it('选择条目不导航时，焦点仍回到触发按钮（不死在 body）', () => {
    const onSelect = vi.fn();
    render(<Menu groups={makeGroups(onSelect)} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole('menuitem', { name: /清理无效路径/ }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(trigger);
  });
});
