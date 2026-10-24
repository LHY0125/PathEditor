import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { markModalOpen } from '@/core/modal-state';

/** 可聚焦元素：与浏览器 Tab 顺序一致，排除 disabled 与 tabindex=-1。 */
const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  /** 无障碍名称（必填）：读屏用户靠它知道当前弹窗是什么。 */
  ariaLabel: string;
  /**
   * 是否允许通过 Escape / 点击遮罩关闭。
   *
   * 默认 true；执行不可中断的操作（如备份恢复）时传 false，
   * 避免用户误触遮罩把正在进行的操作“关掉”（操作本身不会中止，但界面失去反馈）。
   */
  dismissible?: boolean;
  children: ReactNode;
}

/**
 * 对话框外壳。
 *
 * 视觉规格由 globals.css 的 .modal-overlay / .modal-surface 提供；
 * 本组件负责三件事：
 * 1. `role="dialog"` + `aria-modal` + 无障碍名称；
 * 2. 打开时把焦点移入、Tab 在弹窗内循环、关闭时把焦点还给触发元素；
 * 3. 打开期间通知 `core/modal-state`，让全局快捷键暂停。
 *
 * 业务内容由调用方决定。
 */
export function Modal({ open, onClose, ariaLabel, dismissible = true, children }: ModalProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);

  // 打开时记录触发元素并把焦点移入；关闭时归还焦点。
  useEffect(() => {
    if (!open) return;
    restoreFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const surface = surfaceRef.current;
    if (surface) {
      const first = surface.querySelector<HTMLElement>(FOCUSABLE);
      (first ?? surface).focus();
    }
    return () => {
      const target = restoreFocusRef.current;
      restoreFocusRef.current = null;
      if (target && document.contains(target)) target.focus();
    };
  }, [open]);

  // 通知全局快捷键：模态框打开期间不得触发 Delete / Ctrl+S 等。
  useEffect(() => {
    if (!open) return;
    return markModalOpen();
  }, [open]);

  // Escape 关闭（不可中断的操作不响应）。
  useEffect(() => {
    if (!open) return;
    const handler = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'Escape' && dismissible) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose, dismissible]);

  if (!open) return null;

  /** Tab / Shift+Tab 在弹窗内循环，焦点不会移到遮罩层后面的页面。 */
  const trapFocus = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'Tab') return;
    const surface = surfaceRef.current;
    if (!surface) return;
    const items = Array.from(surface.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (items.length === 0) {
      e.preventDefault();
      surface.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === surface)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="modal-overlay" onClick={dismissible ? onClose : undefined}>
      <div
        ref={surfaceRef}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        tabIndex={-1}
        className="modal-surface"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={trapFocus}
      >
        {children}
      </div>
    </div>
  );
}
