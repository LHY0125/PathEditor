import { useEffect } from 'react';
import { useAppStore } from '@/store/app-store';

interface KeyboardActions {
  onNew: () => void;
  onSave: () => void;
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
}

/**
 * 全局键盘快捷键
 * Ctrl+N 新建, Ctrl+S 保存, Ctrl+Z 撤销, Ctrl+Y 重做, Delete 删除
 */
export function useKeyboard(actions: KeyboardActions) {
  const isAdmin = useAppStore((s) => s.isAdmin);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // 如果焦点在输入框中，只响应 Escape
      const tag = (e.target as HTMLElement)?.tagName;
      const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';

      if (isInput) {
        if (e.key === 'Escape') {
          (e.target as HTMLElement).blur();
        }
        return;
      }

      if (!isAdmin) return;

      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && e.key === 'z') {
        e.preventDefault();
        actions.onUndo();
      } else if (ctrl && e.key === 'y') {
        e.preventDefault();
        actions.onRedo();
      } else if (ctrl && e.key === 'n') {
        e.preventDefault();
        actions.onNew();
      } else if (ctrl && e.key === 's') {
        e.preventDefault();
        actions.onSave();
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        actions.onDelete();
      } else if (e.key === 'F1') {
        e.preventDefault();
        // 帮助由 AppShell 处理
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [isAdmin, actions]);
}
