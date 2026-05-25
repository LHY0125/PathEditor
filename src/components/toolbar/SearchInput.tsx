import { useRef, useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';

export function SearchInput() {
  const { t } = useTranslation();
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  // Ctrl+F 聚焦搜索框
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
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
      className="px-3 py-1 text-sm rounded border outline-none w-56"
      style={{
        backgroundColor: 'var(--app-list-bg)',
        color: 'var(--app-fg)',
        borderColor: 'var(--app-border)',
      }}
    />
  );
}
