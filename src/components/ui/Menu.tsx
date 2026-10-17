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

/** 键盘可聚焦的菜单条目选择器（disabled 条目渲染为原生 disabled 按钮，天然不可聚焦）。 */
const ITEM_SELECTOR = 'button[role="menuitem"]:not(:disabled)';

/** 焦点移动目标：首项 / 末项 / 下一项 / 上一项。 */
type MoveTarget = 'first' | 'last' | 'next' | 'prev';

/**
 * 无依赖下拉菜单。
 * - 点击触发按钮开合；点击外部 / Escape 关闭；关闭后焦点回到触发按钮。
 * - 键盘导航（spec N4）：打开后焦点落在首个可用条目；
 *   ArrowDown / ArrowUp 在可用条目间循环移动，跳过 disabled；Home / End 跳到首/尾。
 *   Tab 不拦截，浏览器自然移走焦点（菜单保持打开，由失焦/外点关闭）。
 */
export function Menu({ groups, triggerLabel, triggerAriaLabel, footer }: MenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const panelId = useId();

  /** 面板内当前可用的全部条目（DOM 顺序即视觉顺序）。 */
  const enabledItems = useCallback((): HTMLButtonElement[] => {
    const panel = panelRef.current;
    if (!panel) return [];
    return Array.from(panel.querySelectorAll<HTMLButtonElement>(ITEM_SELECTOR));
  }, []);

  /** 在可用条目中移动焦点到指定位置；next / prev 在两端循环。 */
  const moveFocus = useCallback(
    (to: MoveTarget) => {
      const items = enabledItems();
      if (items.length === 0) return;
      if (to === 'first') {
        items[0].focus();
        return;
      }
      if (to === 'last') {
        items[items.length - 1].focus();
        return;
      }
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      // 当前焦点不在条目上（如落在面板本身）时：向下一步从首项开始，向上从尾项开始
      const next =
        index === -1
          ? to === 'next'
            ? 0
            : items.length - 1
          : (index + (to === 'next' ? 1 : -1) + items.length) % items.length;
      items[next].focus();
    },
    [enabledItems],
  );

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

  // 打开后把焦点移到首个可用条目：鼠标打开同样移动，保证后续方向键有焦点基准
  useEffect(() => {
    if (!open) return;
    // 面板在当前点击事件中挂载，同步聚焦（非 rAF 延迟）以避免焦点短暂滞留触发按钮
    moveFocus('first');
  }, [open, moveFocus]);

  // Escape 关闭（含焦点交还触发按钮）；方向键 / Home / End 在可用条目间移动。
  // Tab 不拦截：交给浏览器自然移走焦点。
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          e.preventDefault();
          close();
          break;
        case 'ArrowDown':
          e.preventDefault();
          moveFocus('next');
          break;
        case 'ArrowUp':
          e.preventDefault();
          moveFocus('prev');
          break;
        case 'Home':
          e.preventDefault();
          moveFocus('first');
          break;
        case 'End':
          e.preventDefault();
          moveFocus('last');
          break;
        default:
          break;
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [open, close, moveFocus]);

  // 条目点击：先关面板，再执行回调
  // 焦点交还触发按钮：否则被点掉的条目把焦点带进 <body>，键盘用户丢失位置。
  // 对会打开对话框/切换主题的条目同样成立 —— 这两类动作都会移动或重建焦点。
  const handleItem = (item: MenuItemDef) => {
    if (item.disabled) return;
    close();
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
        // 触发按钮上按 ArrowDown / ArrowUp 同样打开菜单（打开后焦点由上方 effect 落到首项）
        onKeyDown={(e) => {
          if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
          e.preventDefault();
          setOpen(true);
        }}
      >
        {triggerLabel ?? <MoreIcon />}
      </Button>
      {open && (
        <div
          id={panelId}
          role="menu"
          data-testid="menu-panel"
          ref={panelRef}
          className="menu-panel glass"
        >
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
