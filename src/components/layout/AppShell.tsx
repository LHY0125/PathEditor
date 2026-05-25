import { useState, useCallback, useEffect } from 'react';
import { useAppStore, type TabId } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { TargetType } from '@/core/undo-redo';
import { open } from '@tauri-apps/plugin-dialog';
import { importFromContent, exportToJson, flattenImportResult } from '@/core/import-export';
import { StatusBar } from './StatusBar';
import { TitleBar } from './TitleBar';
import { ToolBar } from '@/components/toolbar/ToolBar';
import { PathTable } from '@/components/path-list/PathTable';
import { MergePreview } from '@/components/path-list/MergePreview';
import { PathEditDialog } from '@/components/dialogs/PathEditDialog';
import { HelpDialog } from '@/components/dialogs/HelpDialog';
import { ImportDialog } from '@/components/dialogs/ImportDialog';
import { useKeyboard } from '@/hooks/use-keyboard';

export function AppShell() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const selectedIndices = useAppStore((s) => s.selectedIndices);
  const setSelectedIndices = useAppStore((s) => s.setSelectedIndices);

  // 对话弹窗状态
  const [editDialog, setEditDialog] = useState<{ open: boolean; index: number; value: string; target: TargetType }>({
    open: false, index: -1, value: '', target: TargetType.SYSTEM,
  });
  const [newDialog, setNewDialog] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [importDialog, setImportDialog] = useState<{ open: boolean; system: string[]; user: string[] }>({
    open: false, system: [], user: [],
  });

  // ── 操作处理 ──

  const getCurrentTarget = useCallback((): TargetType => {
    return activeTab === 'user' ? TargetType.USER : TargetType.SYSTEM;
  }, [activeTab]);

  const handleNew = useCallback(() => {
    setNewDialog(true);
  }, []);

  const handleEdit = useCallback(() => {
    if (selectedIndices.length !== 1) return;
    const idx = selectedIndices[0];
    const target = getCurrentTarget();
    const list = target === TargetType.SYSTEM
      ? useAppStore.getState().sysPaths
      : useAppStore.getState().userPaths;
    const value = list[idx];
    if (value) {
      setEditDialog({ open: true, index: idx, value, target });
    }
  }, [selectedIndices, getCurrentTarget]);

  const handleBrowse = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      useAppStore.getState().addPath(selected, getCurrentTarget());
    }
  }, [getCurrentTarget]);

  const handleDelete = useCallback(() => {
    if (selectedIndices.length === 0) return;
    useAppStore.getState().deletePaths(selectedIndices, getCurrentTarget());
  }, [selectedIndices, getCurrentTarget]);

  const handleMoveUp = useCallback(() => {
    if (selectedIndices.length !== 1) return;
    useAppStore.getState().moveUp(selectedIndices[0], getCurrentTarget());
  }, [selectedIndices, getCurrentTarget]);

  const handleMoveDown = useCallback(() => {
    if (selectedIndices.length !== 1) return;
    useAppStore.getState().moveDown(selectedIndices[0], getCurrentTarget());
  }, [selectedIndices, getCurrentTarget]);

  const handleClean = useCallback(() => {
    const removed = useAppStore.getState().cleanPaths(
      getCurrentTarget(),
      (p) => p.includes('%') || p.includes('\\') || p.includes('/') || /^[a-zA-Z]:[/\\]/.test(p),
    );
    if (removed.length > 0) {
      useAppStore.getState().setStatusMessage(
        t('status.deleted', { count: removed.length }),
      );
    }
  }, [getCurrentTarget, t]);

  const handleImport = useCallback(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,.csv,.txt';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) { input.remove(); return; }
      const content = await file.text();
      const result = importFromContent(content, file.name);
      input.remove();

      if (result.system.length > 0 && result.user.length > 0) {
        setImportDialog({ open: true, system: result.system, user: result.user });
      } else if (result.system.length > 0) {
        useAppStore.getState().importPaths(TargetType.SYSTEM, result.system);
      } else if (result.user.length > 0) {
        useAppStore.getState().importPaths(TargetType.USER, result.user);
      }
    };
    input.click();
  }, []);

  const handleExport = useCallback(() => {
    const state = useAppStore.getState();
    const data = { system: state.sysPaths, user: state.userPaths };

    const content = exportToJson(data);
    const mime = 'application/json';
    const ext = '.json';

    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `patheditor_export${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const handleSave = useCallback(() => {
    useAppStore.getState().savePaths();
  }, []);

  // ── 键盘快捷键 ──

  useKeyboard({
    onNew: handleNew,
    onSave: handleSave,
    onDelete: handleDelete,
    onUndo: () => useAppStore.getState().undo(),
    onRedo: () => useAppStore.getState().redo(),
    onHelp: () => setHelpOpen(true),
  });

  // ── 双击编辑监听 ──

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail && typeof detail.index === 'number') {
        const target = getCurrentTarget();
        setEditDialog({ open: true, index: detail.index, value: detail.path, target });
      }
    };
    window.addEventListener('path-dblclick', handler);
    return () => window.removeEventListener('path-dblclick', handler);
  }, [getCurrentTarget]);

  const handleNewConfirm = useCallback((value: string) => {
    setNewDialog(false);
    if (value.trim()) {
      useAppStore.getState().addPath(value.trim(), getCurrentTarget());
    }
  }, [getCurrentTarget]);

  const handleEditConfirm = useCallback((value: string) => {
    setEditDialog({ open: false, index: -1, value: '', target: TargetType.SYSTEM });
    if (value.trim()) {
      useAppStore.getState().editPath(editDialog.index, value.trim(), editDialog.target);
    }
  }, [editDialog]);

  const handleImportSelect = useCallback((target: 'system' | 'user' | 'both') => {
    const { system, user } = importDialog;
    const flat = flattenImportResult({ system, user }, target);
    if (flat.system.length > 0) {
      useAppStore.getState().importPaths(TargetType.SYSTEM, flat.system);
    }
    if (flat.user.length > 0) {
      useAppStore.getState().importPaths(TargetType.USER, flat.user);
    }
    setImportDialog({ open: false, system: [], user: [] });
  }, [importDialog]);

  // Tab 切换
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

      {/* Tab 栏 */}
      <div
        className="flex border-b px-4"
        style={{ borderColor: 'var(--app-border)' }}
      >
        {tabConfig.map((tab) => (
          <button
            key={tab.id}
            onClick={() => {
              setActiveTab(tab.id);
              setSelectedIndices([]);
            }}
            className={`px-4 py-1.5 text-sm font-medium transition-colors ${
              activeTab === tab.id ? 'tab-active' : 'opacity-60'
            }`}
            style={{ color: activeTab === tab.id ? '#3b82f6' : 'var(--app-fg)' }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* 工具栏 */}
      <div className="px-4 py-2">
        <ToolBar
          onNew={handleNew}
          onEdit={handleEdit}
          onBrowse={handleBrowse}
          onDelete={handleDelete}
          onMoveUp={handleMoveUp}
          onMoveDown={handleMoveDown}
          onClean={handleClean}
          onImport={handleImport}
          onExport={handleExport}
          onSave={handleSave}
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
          onDarkMode={() => useThemeStore.getState().toggle()}
        />
      </div>

      {/* 路径列表（支持拖拽文件夹） */}
      <div
        className="flex-1 overflow-hidden"
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'link';
        }}
        onDrop={(e) => {
          e.preventDefault();
          if (activeTab === 'merged') return;
          const files = e.dataTransfer.files;
          for (let i = 0; i < files.length; i++) {
            const path = (files[i] as any).path;
            if (path) {
              useAppStore.getState().addPath(path, getCurrentTarget());
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

      {/* 弹窗 */}
      <PathEditDialog
        open={newDialog}
        title={t('dialog.newPath')}
        initialValue=""
        onConfirm={handleNewConfirm}
        onCancel={() => setNewDialog(false)}
      />

      <PathEditDialog
        open={editDialog.open}
        title={t('dialog.editPath')}
        initialValue={editDialog.value}
        onConfirm={handleEditConfirm}
        onCancel={() => setEditDialog({ open: false, index: -1, value: '', target: TargetType.SYSTEM })}
      />

      <HelpDialog open={helpOpen} onClose={() => setHelpOpen(false)} />

      <ImportDialog
        open={importDialog.open}
        systemCount={importDialog.system.length}
        userCount={importDialog.user.length}
        onSelect={handleImportSelect}
        onCancel={() => setImportDialog({ open: false, system: [], user: [] })}
      />
    </div>
  );
}
