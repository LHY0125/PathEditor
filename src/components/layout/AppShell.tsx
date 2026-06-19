import { useState } from 'react';
import { useAppStore, type TabId } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { TargetType } from '@/core/undo-redo';
import { StatusBar } from './StatusBar';
import { TitleBar } from './TitleBar';
import { ToolBar } from '@/components/toolbar/ToolBar';
import { PathTable } from '@/components/path-list/PathTable';
import { MergePreview } from '@/components/path-list/MergePreview';
import { PathEditDialog } from '@/components/dialogs/PathEditDialog';
import { HelpDialog } from '@/components/dialogs/HelpDialog';
import { ImportDialog } from '@/components/dialogs/ImportDialog';
import { AnalyzeDialog } from '@/components/dialogs/AnalyzeDialog';
import { ProfileDialog } from '@/components/dialogs/ProfileDialog';
import { useAppActions, type DialogState } from '@/hooks/use-app-actions';

/** Tauri's File object includes the native filesystem path */
interface TauriFile extends File {
  path: string;
}

export function AppShell() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const setSelectedIndices = useAppStore((s) => s.setSelectedIndices);

  const [editDialog, setEditDialog] = useState<DialogState['editDialog']>({
    open: false,
    index: -1,
    value: '',
    target: TargetType.SYSTEM,
  });
  const [newDialog, setNewDialog] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [importDialog, setImportDialog] = useState<DialogState['importDialog']>({
    open: false,
    system: [],
    user: [],
  });
  const [analyzeOpen, setAnalyzeOpen] = useState(false);
  const [profilesOpen, setProfilesOpen] = useState(false);

  const actions = useAppActions(activeTab, {
    editDialog,
    newDialog,
    helpOpen,
    importDialog,
    setEditDialog,
    setNewDialog,
    setHelpOpen,
    setImportDialog,
    setAnalyzeOpen,
    setProfilesOpen,
  });

  const tabConfig: { id: TabId; label: string }[] = [
    { id: 'system', label: t('tab.system') },
    { id: 'user', label: t('tab.user') },
    { id: 'merged', label: t('tab.merged') },
  ];

  return (
    <div
      className="flex flex-col h-screen"
      style={{ backgroundColor: 'var(--app-bg)', color: 'var(--app-fg)' }}
    >
      <TitleBar />

      <div className="flex border-b px-4" style={{ borderColor: 'var(--app-border)' }}>
        {tabConfig.map((tab) => (
          <button
            key={tab.id}
            onClick={() => {
              setActiveTab(tab.id);
              setSelectedIndices([]);
            }}
            className={`px-4 py-1.5 text-sm font-medium transition-colors ${activeTab === tab.id ? 'tab-active' : 'opacity-60'}`}
            style={{ color: activeTab === tab.id ? '#3b82f6' : 'var(--app-fg)' }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="px-4 py-2">
        <ToolBar
          onNew={actions.handleNew}
          onEdit={actions.handleEdit}
          onBrowse={actions.handleBrowse}
          onDelete={actions.handleDelete}
          onMoveUp={actions.handleMoveUp}
          onMoveDown={actions.handleMoveDown}
          onClean={actions.handleClean}
          onImport={actions.handleImport}
          onExport={actions.handleExport}
          onSave={actions.handleSave}
          onCancel={() => {
            const state = useAppStore.getState();
            if (state.isModified && !window.confirm('有未保存的修改，确定退出吗？')) return;
            window.close();
          }}
          onHelp={() => setHelpOpen(true)}
          onLanguage={() => {
            const current = localStorage.getItem('i18nextLng') || 'zh-CN';
            i18n.changeLanguage(current === 'zh-CN' ? 'en' : 'zh-CN');
          }}
          onProfiles={() => setProfilesOpen(true)}
          onAnalyze={() => setAnalyzeOpen(true)}
          onDarkMode={() => useThemeStore.getState().toggle()}
        />
      </div>

      <div
        className="flex-1 overflow-auto"
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'link';
        }}
        onDrop={(e) => {
          e.preventDefault();
          if (activeTab === 'merged') return;
          for (let i = 0; i < e.dataTransfer.items.length; i++) {
            const entry = e.dataTransfer.items[i].webkitGetAsEntry();
            if (entry?.isDirectory) {
              const file = e.dataTransfer.files[i] as TauriFile;
              if (file.path)
                useAppStore
                  .getState()
                  .addPath(file.path, activeTab === 'user' ? TargetType.USER : TargetType.SYSTEM);
            }
          }
        }}
      >
        {activeTab === 'merged' ? (
          <MergePreview />
        ) : (
          <PathTable tabId={activeTab as 'system' | 'user'} />
        )}
      </div>

      <StatusBar />

      <PathEditDialog
        open={newDialog}
        title={t('dialog.newPath')}
        initialValue=""
        onConfirm={actions.handleNewConfirm}
        onCancel={() => setNewDialog(false)}
      />
      <PathEditDialog
        open={editDialog.open}
        title={t('dialog.editPath')}
        initialValue={editDialog.value}
        onConfirm={actions.handleEditConfirm}
        onCancel={() =>
          setEditDialog({ open: false, index: -1, value: '', target: TargetType.SYSTEM })
        }
      />
      <HelpDialog open={helpOpen} onClose={() => setHelpOpen(false)} />
      <ImportDialog
        open={importDialog.open}
        systemCount={importDialog.system.length}
        userCount={importDialog.user.length}
        onSelect={actions.handleImportSelect}
        onCancel={() => setImportDialog({ open: false, system: [], user: [] })}
      />
      <AnalyzeDialog open={analyzeOpen} onClose={() => setAnalyzeOpen(false)} />
      <ProfileDialog open={profilesOpen} onClose={() => setProfilesOpen(false)} />
    </div>
  );
}
