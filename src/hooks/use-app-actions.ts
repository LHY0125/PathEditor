import { useCallback, useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { TargetType } from '@/core/undo-redo';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { importFromContent, exportToJson, exportToCsv, flattenImportResult } from '@/core/import-export';
import type { PathEntry } from '@/core/path-entry';
import { is_valid_path_format } from '@/core/validation';
import { useKeyboard } from './use-keyboard';
import i18n from '@/i18n';
import type { TabId } from '@/store/app-store';

export interface DialogState {
  editDialog: { open: boolean; index: number; value: string; target: TargetType };
  newDialog: boolean;
  helpOpen: boolean;
  importDialog: { open: boolean; system: PathEntry[]; user: PathEntry[] };
  setEditDialog: (v: DialogState['editDialog']) => void;
  setNewDialog: (v: boolean) => void;
  setHelpOpen: (v: boolean) => void;
  setImportDialog: (v: DialogState['importDialog']) => void;
  setAnalyzeOpen: (v: boolean) => void;
  setProfilesOpen: (v: boolean) => void;
}

export function useAppActions(activeTab: TabId, dialogs: DialogState) {
  const { setEditDialog, setNewDialog, setHelpOpen, setImportDialog } = dialogs;

  const getCurrentTarget = useCallback((): TargetType => {
    return activeTab === 'user' ? TargetType.USER : TargetType.SYSTEM;
  }, [activeTab]);

  // ── CRUD ──

  const handleNew = useCallback(() => setNewDialog(true), [setNewDialog]);

  const handleEdit = useCallback(() => {
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    const target = activeTab === 'user' ? TargetType.USER : TargetType.SYSTEM;
    const list = target === TargetType.SYSTEM
      ? useAppStore.getState().sysPaths
      : useAppStore.getState().userPaths;
    const entry = list[idx];
    if (entry) setEditDialog({ open: true, index: idx, value: entry.path, target });
  }, [activeTab, setEditDialog]);

  const handleBrowse = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      useAppStore.getState().addPath(selected, getCurrentTarget());
    }
  }, [getCurrentTarget]);

  const handleDelete = useCallback(() => {
    const idx = useAppStore.getState().selectedIndices;
    if (idx.length === 0) return;
    useAppStore.getState().deletePaths(idx, getCurrentTarget());
  }, [getCurrentTarget]);

  const handleMoveUp = useCallback(() => {
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    useAppStore.getState().moveUp(idx, getCurrentTarget());
  }, [getCurrentTarget]);

  const handleMoveDown = useCallback(() => {
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    useAppStore.getState().moveDown(idx, getCurrentTarget());
  }, [getCurrentTarget]);

  const handleClean = useCallback(() => {
    const removed = useAppStore.getState().cleanPaths(
      getCurrentTarget(),
      is_valid_path_format,
    );
    if (removed.length > 0) {
      useAppStore.getState().setStatusMessage(
        i18n.t('status.deleted', { count: removed.length }),
      );
    }
  }, [getCurrentTarget]);

  // ── 导入导出 ──

  const handleImport = useCallback(async () => {
    const selected = await open({
      filters: [{ name: '受支持格式', extensions: ['json', 'csv', 'txt'] }],
      multiple: false,
    });
    if (!selected || typeof selected !== 'string') return;
    const content = await invoke<string>('read_text_file', { path: selected });
    const result = importFromContent(content, selected);
    if (result.system.length > 0 && result.user.length > 0) {
      setImportDialog({ open: true, system: result.system, user: result.user });
    } else if (result.system.length > 0) {
      useAppStore.getState().replacePaths(TargetType.SYSTEM, result.system.map(e => e.path));
    } else if (result.user.length > 0) {
      useAppStore.getState().replacePaths(TargetType.USER, result.user.map(e => e.path));
    }
  }, [setImportDialog]);

  const handleExport = useCallback((format: 'json' | 'csv' = 'json') => {
    const state = useAppStore.getState();
    const data = { system: state.sysPaths, user: state.userPaths };
    const isCsv = format === 'csv';
    const content = isCsv ? exportToCsv(data) : exportToJson(data);
    const mime = isCsv ? 'text/csv' : 'application/json';
    const ext = isCsv ? '.csv' : '.json';
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `patheditor_export${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const handleSave = useCallback(async () => {
    const saved = await useAppStore.getState().savePaths();
    if (!saved && !useAppStore.getState().isSaving) {
      // 长度超限，需要用户确认
      const { ask } = await import('@tauri-apps/plugin-dialog');
      const confirmed = await ask(i18n.t('status.saveWarningLongPaths'), { title: i18n.t('dialog.backupTitle'), kind: 'warning' });
      if (confirmed) {
        await useAppStore.getState().savePaths(true);
      }
    }
  }, []);

  // ── 键盘 ──

  useKeyboard({
    onNew: handleNew,
    onSave: handleSave,
    onDelete: handleDelete,
    onUndo: () => useAppStore.getState().undo(),
    onRedo: () => useAppStore.getState().redo(),
    onHelp: () => setHelpOpen(true),
  });

  // ── 双击编辑 ──

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
  }, [getCurrentTarget, setEditDialog]);

  // ── 弹窗确认 ──

  const handleNewConfirm = useCallback((value: string) => {
    setNewDialog(false);
    if (value.trim()) useAppStore.getState().addPath(value.trim(), getCurrentTarget());
  }, [getCurrentTarget, setNewDialog]);

  const handleEditConfirm = useCallback((value: string) => {
    const d = dialogs.editDialog;
    setEditDialog({ open: false, index: -1, value: '', target: TargetType.SYSTEM });
    if (value.trim()) useAppStore.getState().editPath(d.index, value.trim(), d.target);
  }, [dialogs.editDialog, setEditDialog]);

  const handleImportSelect = useCallback((target: 'system' | 'user' | 'both') => {
    const { system, user } = dialogs.importDialog;
    const flat = flattenImportResult({ system, user }, target);
    if (target === 'both' && flat.system.length > 0 && flat.user.length > 0) {
      useAppStore.getState().replaceBothPaths(flat.system.map(e => e.path), flat.user.map(e => e.path));
    } else {
      if (flat.system.length > 0) useAppStore.getState().replacePaths(TargetType.SYSTEM, flat.system.map(e => e.path));
      if (flat.user.length > 0) useAppStore.getState().replacePaths(TargetType.USER, flat.user.map(e => e.path));
    }
    setImportDialog({ open: false, system: [], user: [] });
  }, [dialogs.importDialog, setImportDialog]);

  return {
    handleNew, handleEdit, handleBrowse, handleDelete,
    handleMoveUp, handleMoveDown, handleClean,
    handleImport, handleExport, handleSave,
    handleNewConfirm, handleEditConfirm, handleImportSelect,
  };
}
