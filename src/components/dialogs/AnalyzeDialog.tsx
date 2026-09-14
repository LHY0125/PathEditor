import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { ConflictsTab } from './analyze/ConflictsTab';
import { ToolsTab } from './analyze/ToolsTab';
import { useAnalyzeData } from './analyze/use-analyze-data';

type TabType = 'conflicts' | 'tools';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AnalyzeDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<TabType>('conflicts');
  const { loading, error, conflicts, filteredTools, searchQuery, setSearchQuery } =
    useAnalyzeData(open);

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxHeight: '75vh' }}>
        <div
          className="flex items-center justify-between px-5 py-3 border-b"
          style={{ borderColor: 'var(--app-border)' }}
        >
          <h2 className="text-base font-semibold">{t('analyze.title')}</h2>
          <div className="flex gap-1">
            {(['conflicts', 'tools'] as TabType[]).map((tabId) => (
              <button
                key={tabId}
                onClick={() => setTab(tabId)}
                className="px-3 py-1 text-sm rounded transition-colors"
                style={{
                  backgroundColor: tab === tabId ? '#3b82f6' : 'transparent',
                  color: tab === tabId ? '#fff' : 'var(--app-fg)',
                }}
              >
                {tabId === 'conflicts' ? t('analyze.conflicts') : t('analyze.tools')}
              </button>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-auto p-4">
          {loading ? (
            <div
              className="flex items-center justify-center py-12 text-sm"
              style={{ color: 'var(--app-fg)', opacity: 0.6 }}
            >
              {t('analyze.scanning')}
            </div>
          ) : error ? (
            <div className="text-center py-12 text-sm text-red-500">
              {t('analyze.error')}: {error}
            </div>
          ) : tab === 'conflicts' ? (
            <ConflictsTab conflicts={conflicts} />
          ) : (
            <ToolsTab groups={filteredTools} query={searchQuery} onQueryChange={setSearchQuery} />
          )}
        </div>
      </div>
    </Modal>
  );
}
