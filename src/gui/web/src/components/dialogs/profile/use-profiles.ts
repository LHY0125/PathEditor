import { useCallback, useEffect, useRef, useState } from 'react';
import { ask } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import appConfig from '@/config/default.json';
import { useAppStore } from '@/store/app-store';
import { backend, type ProfileView, type ProfilePathsView } from '@/services/backend';
import { errorMessage } from '@/core/error';

export function useProfiles(open: boolean, onClose: () => void) {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [newName, setNewName] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedData, setSelectedData] = useState<ProfilePathsView | null>(null);
  const [saving, setSaving] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameValue, setRenameValue] = useState('');
  const prevOpen = useRef(false);

  const refreshProfiles = useCallback(async () => {
    setProfiles(await backend.profileList());
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
      // 方案 A：写 DB（快照 + profile 行同一事务），不再写 profiles/*.json
      await backend.profileSave({
        name: newName.trim(),
        description: null,
        tags: [],
        system: sysPaths,
        user: userPaths,
      });
      setNewName('');
      await refreshProfiles();
    } finally {
      setSaving(false);
    }
  }, [newName, refreshProfiles]);

  const handleLoad = useCallback(async (name: string) => {
    setSelected(name);
    setSelectedData(await backend.profilePaths(name));
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

    // 组合长度是 UX 软提醒（确认后可继续）；单 hive 的 32767 硬上限由 core 校验。
    const enabled = [...selectedData.system, ...selectedData.user].filter((entry) => entry.enabled);
    const combinedLength = enabled.reduce(
      (sum, entry, index) => sum + entry.path.length + (index > 0 ? 1 : 0),
      0,
    );
    if (combinedLength > appConfig.path.maxCombinedLength) {
      const confirmed = await ask(t('status.saveWarningLongPaths'), {
        title: t('dialog.backupTitle'),
        kind: 'warning',
      });
      if (!confirmed) return;
    }

    // 读配置、写注册表、记账全部在 core；这里只负责确认与结果呈现。
    try {
      await backend.profileApply(selected);
      onClose();
    } catch (error) {
      useAppStore.getState().setStatusError(`${t('status.error')}: ${errorMessage(error)}`);
    }
  }, [onClose, selected, selectedData, t]);

  const handleDelete = useCallback(
    async (name: string) => {
      // 异步确认替代阻塞式 window.confirm；IPC 失败按「取消」处理（fail-closed），
      // 绝不静默删除配置文件。
      if (!(await backend.confirmDialog(t('profile.deleteConfirm', { name })).catch(() => false))) {
        return;
      }
      await backend.profileDeleteByName(name);
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
    await backend.profileRename(selected, nextName);
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
