import { useEffect, useRef } from 'react';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';

interface KeyboardActions {
  onNew: () => void;
  onSave: () => void;
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onHelp: () => void;
}

/**
 * 全局键盘快捷键
 * Ctrl+N 新建, Ctrl+S 保存, Ctrl+Z 撤销, Ctrl+Y 重做, Delete 删除, F1 帮助
 * 使用 ref 避免因 actions 对象每次渲染都是新引用而重复注册事件
 */
export function useKeyboard(actions: KeyboardActions) {
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);
  const actionsRef = useRef(actions);
  // eslint-disable-next-line react-hooks/refs -- React 官方推荐的 ref 同步模式，避免每次渲染重复注册事件监听器
  actionsRef.current = actions;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';

      if (isInput) {
        if (e.key === 'Escape') {
          (e.target as HTMLElement).blur();
        }
        return;
      }

      const a = actionsRef.current;
      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && e.key === 'z') {
        if (!canWrite) return;
        e.preventDefault();
        a.onUndo();
      } else if (ctrl && e.key === 'y') {
        if (!canWrite) return;
        e.preventDefault();
        a.onRedo();
      } else if (ctrl && e.key === 'n') {
        if (!canWrite) return;
        e.preventDefault();
        a.onNew();
      } else if (ctrl && e.key === 's') {
        if (!canWrite) return;
        e.preventDefault();
        a.onSave();
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        if (!canWrite) return;
        e.preventDefault();
        a.onDelete();
      } else if (e.key === 'F1') {
        e.preventDefault();
        a.onHelp();
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [canWrite]); // actions 通过 ref 读取
}
