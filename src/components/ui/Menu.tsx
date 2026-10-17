import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Button } from './Button';
import { MoreIcon } from './icons';

export type MenuItemDef = {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** 仅展示的快捷键提示，如 'Ctrl+P' */
  shortcut?: string;
  /** 破坏性操作（红色） */
  danger?: boolean;
};

export type MenuGroupDef = { key: string; label?: string; items: MenuItemDef[] };

export type MenuProps = {
  groups: MenuGroupDef[];
  /** 触发按钮的可见内容，默认「更多」图标 */
  triggerLabel?: ReactNode;
  /** 触发按钮的无障碍名称（必填） */
  triggerAriaLabel: string;
  /** 底部条目，渲染在分隔线之后（如「取消并关闭」） */
  footer?: MenuItemDef;
};

/**
 * 无依赖下拉菜单。
 * - 点击触发按钮开合；点击外部 / Escape 关闭；关闭后焦点回到触发按钮。
 * - 条目不实现 aria-activedescendant 式键盘漫游，但可用 Tab 键遍历（各条目为原生 button）。
 */
export function Menu({ groups, triggerLabel, triggerAriaLabel, footer }: MenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();

  // 关闭并把焦点交还触发按钮（Escape 与「取消并关闭」等场景）
  const close = useCallback(() => {
    setOpen(false);
    triggerRef.current?.focus();
  }, []);

  // 点击外部关闭；只关面板、不抢焦点，避免打断用户在新位置的点击
  useEffect(() => {
    if (!open) return;
    const onMouseDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onMouseDown);
    return () => document.removeEventListener('mousedown', onMouseDown);
  }, [open]);

  // Escape 关闭
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      close();
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [open, close]);

  // 条目点击：先关面板，再执行回调
  const handleItem = (item: MenuItemDef) => {
    if (item.disabled) return;
    setOpen(false);
    item.onSelect();
  };

  const renderItem = (item: MenuItemDef) => {
    const cls = item.danger ? 'menu-item menu-item-danger' : 'menu-item';
    return (
      <button
        key={item.key}
        type="button"
        role="menuitem"
        disabled={item.disabled}
        onClick={() => handleItem(item)}
        className={cls}
      >
        <span>{item.label}</span>
        {item.shortcut && <span className="menu-shortcut">{item.shortcut}</span>}
      </button>
    );
  };

  return (
    <div className="relative" ref={rootRef}>
      <Button
        ref={triggerRef}
        variant="ghost"
        aria-label={triggerAriaLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        onClick={() => setOpen((v) => !v)}
      >
        {triggerLabel ?? <MoreIcon />}
      </Button>
      {open && (
        <div id={panelId} role="menu" data-testid="menu-panel" className="menu-panel glass">
          {groups.map((group) => (
            <div key={group.key}>
              {group.label && <div className="menu-group-label">{group.label}</div>}
              {group.items.map(renderItem)}
            </div>
          ))}
          {footer && (
            <>
              <div className="menu-separator" />
              {renderItem(footer)}
            </>
          )}
        </div>
      )}
    </div>
  );
}
