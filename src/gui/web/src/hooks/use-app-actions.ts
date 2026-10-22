import { useCallback, useEffect } from 'react';
import { canWriteTarget, targetForTab, useAppStore, type TabId } from '@/store/app-store';
import { TargetType } from '@/core/undo-redo';
import { ask, open } from '@tauri-apps/plugin-dialog';
import type { PathEntry } from '@/core/path-entry';
import { backend } from '@/services/backend';
import { useKeyboard } from './use-keyboard';
import i18n from '@/i18n';

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

  const canWrite = useCallback((target: TargetType) => {
    const state = useAppStore.getState();
    return canWriteTarget(state.isAdmin, state.pathCapabilities, target);
  }, []);

  const canWriteCurrent = useCallback(() => {
    const target = targetForTab(activeTab);
    return target !== null && canWrite(target);
  }, [activeTab, canWrite]);

  // ── CRUD ──

  const handleNew = useCallback(() => {
    if (canWriteCurrent()) setNewDialog(true);
  }, [canWriteCurrent, setNewDialog]);

  const handleEdit = useCallback(() => {
    if (!canWriteCurrent()) return;
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    const target = getCurrentTarget();
    const list =
      target === TargetType.SYSTEM
        ? useAppStore.getState().sysPaths
        : useAppStore.getState().userPaths;
    const entry = list[idx];
    if (entry) setEditDialog({ open: true, index: idx, value: entry.path, target });
  }, [canWriteCurrent, getCurrentTarget, setEditDialog]);

  const handleBrowse = useCallback(async () => {
    if (!canWriteCurrent()) return;
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      useAppStore.getState().addPath(selected, getCurrentTarget());
    }
  }, [canWriteCurrent, getCurrentTarget]);

  const handleDelete = useCallback(() => {
    if (!canWriteCurrent()) return;
    const indices = useAppStore.getState().selectedIndices;
    if (indices.length === 0) return;
    useAppStore.getState().deletePaths(indices, getCurrentTarget());
  }, [canWriteCurrent, getCurrentTarget]);

  const handleMoveUp = useCallback(() => {
    if (!canWriteCurrent()) return;
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    useAppStore.getState().moveUp(idx, getCurrentTarget());
  }, [canWriteCurrent, getCurrentTarget]);

  const handleMoveDown = useCallback(() => {
    if (!canWriteCurrent()) return;
    const idx = useAppStore.getState().selectedIndices[0];
    if (idx === undefined) return;
    useAppStore.getState().moveDown(idx, getCurrentTarget());
  }, [canWriteCurrent, getCurrentTarget]);

  const handleClean = useCallback(async () => {
    if (!canWriteCurrent()) return;
    try {
      const removed = await useAppStore.getState().cleanPaths(getCurrentTarget());
      if (removed.length > 0) {
        useAppStore
          .getState()
          .setStatusMessage(i18n.t('status.deleted', { count: removed.length }));
      }
    } catch (error) {
      useAppStore.getState().setStatusMessage(`${i18n.t('status.error')}: ${String(error)}`);
    }
  }, [canWriteCurrent, getCurrentTarget]);

  // ── 导入导出 ──

  const handleImport = useCallback(async () => {
    if (!canWriteCurrent()) return;
    const selected = await open({
      filters: [{ name: i18n.t('dialog.importFilterName'), extensions: ['json', 'csv', 'txt'] }],
      multiple: false,
    });
    if (!selected || typeof selected !== 'string') return;

    try {
      const [system, user] = await backend.importFile(selected);
      if (system.length > 0 && user.length > 0) {
        setImportDialog({ open: true, system, user });
      } else if (system.length > 0) {
        if (!canWrite(TargetType.SYSTEM)) {
          useAppStore.getState().setStatusMessage(i18n.t('status.noSystemPermission'));
          return;
        }
        useAppStore.getState().replacePaths(TargetType.SYSTEM, system);
      } else if (user.length > 0) {
        if (!canWrite(TargetType.USER)) {
          useAppStore.getState().setStatusMessage(i18n.t('status.noUserPermission'));
          return;
        }
        useAppStore.getState().replacePaths(TargetType.USER, user);
      }
    } catch (error) {
      useAppStore.getState().setStatusMessage(`${i18n.t('status.error')}: ${String(error)}`);
    }
  }, [canWrite, canWriteCurrent, setImportDialog]);

  const handleExport = useCallback(async (format: 'json' | 'csv' = 'json') => {
    const state = useAppStore.getState();
    try {
      const content = await backend.exportPathEntries(state.sysPaths, state.userPaths, format);
      const mime = format === 'csv' ? 'text/csv' : 'application/json';
      const blob = new Blob([content], { type: mime });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `patheditor_export.${format}`;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      state.setStatusMessage(`${i18n.t('status.error')}: ${String(error)}`);
    }
  }, []);

  const handleSave = useCallback(async () => {
    const result = await useAppStore.getState().savePaths();
    if (result.kind === 'warning') {
      const confirmed = await ask(i18n.t('status.saveWarningLongPaths'), {
        title: i18n.t('dialog.backupTitle'),
        kind: 'warning',
      });
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
      if (detail && typeof detail.index === 'number' && canWriteCurrent()) {
        const target = getCurrentTarget();
        setEditDialog({ open: true, index: detail.index, value: detail.path, target });
      }
    };
    window.addEventListener('path-dblclick', handler);
    return () => window.removeEventListener('path-dblclick', handler);
  }, [canWriteCurrent, getCurrentTarget, setEditDialog]);

  // ── 弹窗确认 ──

  const handleNewConfirm = useCallback(
    (value: string) => {
      setNewDialog(false);
      if (value.trim() && canWriteCurrent()) {
        useAppStore.getState().addPath(value.trim(), getCurrentTarget());
      }
    },
    [canWriteCurrent, getCurrentTarget, setNewDialog],
  );

  const handleEditConfirm = useCallback(
    (value: string) => {
      const dialog = dialogs.editDialog;
      setEditDialog({ open: false, index: -1, value: '', target: TargetType.SYSTEM });
      if (value.trim() && canWrite(dialog.target)) {
        useAppStore.getState().editPath(dialog.index, value.trim(), dialog.target);
      }
    },
    [canWrite, dialogs.editDialog, setEditDialog],
  );

  const handleImportSelect = useCallback(
    (target: 'system' | 'user' | 'both') => {
      const { system, user } = dialogs.importDialog;
      if (target === 'both') {
        const missing: string[] = [];
        if (!canWrite(TargetType.SYSTEM)) missing.push(i18n.t('status.noSystemPermission'));
        if (!canWrite(TargetType.USER)) missing.push(i18n.t('status.noUserPermission'));
        if (missing.length > 0) {
          useAppStore.getState().setStatusMessage(missing.join('; '));
          return;
        }
        useAppStore.getState().replaceBothPaths(system, user);
      } else {
        const targetType = target === 'system' ? TargetType.SYSTEM : TargetType.USER;
        const entries = target === 'system' ? system : user;
        if (!canWrite(targetType)) {
          const key = target === 'system' ? 'status.noSystemPermission' : 'status.noUserPermission';
          useAppStore.getState().setStatusMessage(i18n.t(key));
          return;
        }
        if (entries.length > 0) {
          useAppStore.getState().replacePaths(targetType, entries);
        }
      }
      setImportDialog({ open: false, system: [], user: [] });
    },
    [canWrite, dialogs.importDialog, setImportDialog],
  );

  return {
    handleNew,
    handleEdit,
    handleBrowse,
    handleDelete,
    handleMoveUp,
    handleMoveDown,
    handleClean,
    handleImport,
    handleExport,
    handleSave,
    handleNewConfirm,
    handleEditConfirm,
    handleImportSelect,
  };
}
