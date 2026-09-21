import { useCallback, useEffect, useRef, useState } from 'react';
import { ask } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
import { backend, type ProfileData, type ProfileMeta } from '@/services/backend';

export function useProfiles(open: boolean, onClose: () => void) {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [newName, setNewName] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedData, setSelectedData] = useState<ProfileData | null>(null);
  const [saving, setSaving] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameValue, setRenameValue] = useState('');
  const prevOpen = useRef(false);

  const refreshProfiles = useCallback(async () => {
    setProfiles(await backend.listProfiles());
  }, []);

  useEffect(() => {
    if (open && !prevOpen.current) void refreshProfiles();
    prevOpen.current = open;
  }, [open, refreshProfiles]);

  const handleSave = useCallback(async () => {
    if (!newName.trim()) return;
    setSaving(true);
    try {
      const { sysPaths, userPaths } = useAppStore.getState();
      await backend.saveProfile(newName.trim(), sysPaths, userPaths);
      setNewName('');
      await refreshProfiles();
    } finally {
      setSaving(false);
    }
  }, [newName, refreshProfiles]);

  const handleLoad = useCallback(async (name: string) => {
    setSelected(name);
    setSelectedData(await backend.loadProfile(name));
  }, []);

  const handleApply = useCallback(async () => {
    if (!selected || !selectedData) return;
    // 异步确认替代阻塞式 window.confirm（同关窗路径机制）。应用配置会覆盖 PATH
    // 并写注册表，属破坏性操作：对话框 IPC 失败按「取消」处理（fail-closed）。
    if (
      !(await backend
        .confirmDialog(t('profile.applyConfirm', { name: selected }))
        .catch(() => false))
    ) {
      return;
    }

    useAppStore.getState().replaceBothPaths(selectedData.sys, selectedData.user);
    const result = await useAppStore.getState().savePaths();
    if (result.kind === 'success') {
      onClose();
      return;
    }
    if (result.kind !== 'warning') return;

    const confirmed = await ask(t('status.saveWarningLongPaths'), {
      title: t('dialog.backupTitle'),
      kind: 'warning',
    });
    if (confirmed && (await useAppStore.getState().savePaths(true)).kind === 'success') {
      onClose();
    }
  }, [onClose, selected, selectedData, t]);

  const handleDelete = useCallback(
    async (name: string) => {
      // 异步确认替代阻塞式 window.confirm；IPC 失败按「取消」处理（fail-closed），
      // 绝不静默删除配置文件。
      if (!(await backend.confirmDialog(t('profile.deleteConfirm', { name })).catch(() => false))) {
        return;
      }
      await backend.deleteProfile(name);
      if (selected === name) {
        setSelected(null);
        setSelectedData(null);
      }
      await refreshProfiles();
    },
    [refreshProfiles, selected, t],
  );

  const handleRename = useCallback(async () => {
    if (!selected || !renameValue.trim()) return;
    const nextName = renameValue.trim();
    await backend.renameProfile(selected, nextName);
    setRenameOpen(false);
    setSelected(nextName);
    await refreshProfiles();
  }, [refreshProfiles, renameValue, selected]);

  return {
    profiles,
    newName,
    setNewName,
    selected,
    selectedData,
    saving,
    renameOpen,
    setRenameOpen,
    renameValue,
    setRenameValue,
    handleSave,
    handleLoad,
    handleApply,
    handleDelete,
    handleRename,
  };
}
