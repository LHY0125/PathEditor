import { useEffect, type ReactNode } from 'react';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

/**
 * 对话框外壳。
 *
 * 视觉规格由 globals.css 的 .modal-overlay / .modal-surface 提供（Wave 4.1），
 * 组件本身只管挂载与 Escape 关闭 —— 业务内容由调用方决定。
 */
export function Modal({ open, onClose, children }: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-surface" onClick={(e) => e.stopPropagation()}>
        {children}
      </div>
    </div>
  );
}
