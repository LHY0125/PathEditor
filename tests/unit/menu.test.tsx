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
    // 快捷键提示挂在对应条目的 .menu-shortcut 上（仅断言存在会漏掉挂错条目）
    const helpItem = screen.getByRole('menuitem', { name: /帮助/ });
    expect(helpItem.querySelector('.menu-shortcut')?.textContent).toBe('F1');
  });

  it('aria-expanded 随开合切换', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
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

describe('Menu 键盘导航', () => {
  /** 两个分组共四个条目，其中「条目乙」禁用，用于验证焦点跳过。 */
  function keyboardGroups(): MenuGroupDef[] {
    const noop = vi.fn();
    return [
      {
        key: 'g1',
        items: [
          { key: 'a', label: '条目甲', onSelect: noop },
          { key: 'b', label: '条目乙', onSelect: noop, disabled: true },
          { key: 'c', label: '条目丙', onSelect: noop },
        ],
      },
      {
        key: 'g2',
        items: [{ key: 'd', label: '条目丁', onSelect: noop }],
      },
    ];
  }

  /** 打开菜单（点击触发按钮），返回触发按钮。 */
  function openMenu(): HTMLElement {
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    return trigger;
  }

  const item = (name: string) => screen.getByRole('menuitem', { name });
  const press = (key: string) => fireEvent.keyDown(document, { key });

  it('打开后焦点落在首个可用条目', () => {
    render(
      <Menu
        groups={keyboardGroups()}
        triggerAriaLabel="更多操作"
        footer={{ key: 'cancel', label: '取消并关闭', onSelect: vi.fn() }}
      />,
    );
    openMenu();
    expect(document.activeElement).toBe(item('条目甲'));
  });

  it('首项禁用时跳过，焦点落在首个可用条目', () => {
    render(
      <Menu
        groups={[
          {
            key: 'g',
            items: [
              { key: 'a', label: '禁用首项', onSelect: vi.fn(), disabled: true },
              { key: 'b', label: '可用项', onSelect: vi.fn() },
            ],
          },
        ]}
        triggerAriaLabel="更多操作"
      />,
    );
    openMenu();
    expect(document.activeElement).toBe(item('可用项'));
  });

  it('ArrowDown 移到下一个可用条目，并跳过 disabled', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    openMenu();
    press('ArrowDown');
    // 乙 是 disabled —— 必须被跳过，直接落到 丙
    expect(document.activeElement).toBe(item('条目丙'));
    press('ArrowDown');
    expect(document.activeElement).toBe(item('条目丁'));
  });

  it('ArrowUp 从首项循环到末项', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    openMenu();
    press('ArrowUp');
    expect(document.activeElement).toBe(item('条目丁'));
    press('ArrowUp');
    // 末项向上：跳过 disabled 的 乙，落到 丙
    expect(document.activeElement).toBe(item('条目丙'));
  });

  it('ArrowDown 从末项循环回首项', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    openMenu();
    press('End');
    press('ArrowDown');
    expect(document.activeElement).toBe(item('条目甲'));
  });

  it('Home / End 跳到首尾可用条目', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    openMenu();
    press('End');
    expect(document.activeElement).toBe(item('条目丁'));
    press('Home');
    expect(document.activeElement).toBe(item('条目甲'));
  });

  it('footer 条目参与循环（排在末项）', () => {
    render(
      <Menu
        groups={keyboardGroups()}
        triggerAriaLabel="更多操作"
        footer={{ key: 'cancel', label: '取消并关闭', onSelect: vi.fn() }}
      />,
    );
    openMenu();
    press('End');
    expect(document.activeElement).toBe(item('取消并关闭'));
    press('ArrowDown');
    expect(document.activeElement).toBe(item('条目甲'));
  });

  it('Escape 关闭菜单并把焦点还给触发按钮', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    const trigger = openMenu();
    press('ArrowDown');
    press('Escape');
    expect(screen.queryByTestId('menu-panel')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('Tab 不被拦截（菜单保持打开，焦点可自然移出）', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    openMenu();
    const event = new KeyboardEvent('keydown', { key: 'Tab', cancelable: true });
    document.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
    expect(screen.getByTestId('menu-panel')).toBeInTheDocument();
  });

  it('触发按钮上按 ArrowDown 打开菜单并聚焦首项', () => {
    render(<Menu groups={keyboardGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.keyDown(trigger, { key: 'ArrowDown' });
    expect(screen.getByTestId('menu-panel')).toBeInTheDocument();
    expect(document.activeElement).toBe(item('条目甲'));
  });
});
