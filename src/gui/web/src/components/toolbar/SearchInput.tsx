import { useRef, useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { isAnyModalOpen } from '@/core/modal-state';
import { useTranslation } from 'react-i18next';

export function SearchInput() {
  const { t } = useTranslation();
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  // Ctrl+F 聚焦搜索框
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // 模态框打开时把焦点移到遮罩背后会破坏焦点陷阱（键盘用户困在弹窗外）。
      if (isAnyModalOpen()) return;
      if (e.ctrlKey && e.key === 'f') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  return (
    <input
      ref={inputRef}
      type="text"
      placeholder={t('dialog.search')}
      value={searchQuery}
      onChange={(e) => setSearchQuery(e.target.value)}
      aria-label={t('dialog.search')}
      className="toolbar-search"
    />
  );
}
