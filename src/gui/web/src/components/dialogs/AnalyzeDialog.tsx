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
      <div className="flex flex-col" style={{ width: 680, maxWidth: '100%', maxHeight: '75vh' }}>
        <div
          className="flex items-center justify-between px-1 py-2"
          style={{ borderBottom: '1px solid var(--app-border)' }}
        >
          <h2 className="text-base font-semibold">{t('analyze.title')}</h2>
          <div className="flex gap-1">
            {(['conflicts', 'tools'] as TabType[]).map((tabId) => (
              <button
                key={tabId}
                type="button"
                data-active={tab === tabId}
                className="tab-pill text-xs"
                onClick={() => setTab(tabId)}
              >
                {tabId === 'conflicts' ? t('analyze.conflicts') : t('analyze.tools')}
              </button>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-auto p-3">
          {loading ? (
            <div
              className="flex items-center justify-center py-12 text-xs"
              style={{ color: 'var(--app-fg-muted)' }}
            >
              {t('analyze.scanning')}
            </div>
          ) : error ? (
            <div className="text-center py-12 text-xs" style={{ color: 'var(--app-danger)' }}>
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
