import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { useAppStore } from '@/store/app-store';
import type { PathEntry } from '@/core/path-entry';

interface ProfileMeta {
  name: string;
  created: string;
  modified: string;
}

interface ProfileData {
  name: string;
  sys: PathEntry[];
  user: PathEntry[];
  created: string;
  modified: string;
}

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ProfileDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [newName, setNewName] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedData, setSelectedData] = useState<ProfileData | null>(null);
  const [saving, setSaving] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameValue, setRenameValue] = useState('');

  const refreshProfiles = useCallback(async () => {
    const list = await invoke<ProfileMeta[]>('list_profiles');
    setProfiles(list);
  }, []);

  useEffect(() => {
    if (open) refreshProfiles();
  }, [open, refreshProfiles]);

  const handleSave = async () => {
    if (!newName.trim()) return;
    setSaving(true);
    const { sysPaths, userPaths } = useAppStore.getState();
    await invoke('save_profile', { name: newName.trim(), sys: sysPaths, user: userPaths });
    setNewName('');
    setSaving(false);
    refreshProfiles();
  };

  const handleLoad = async (name: string) => {
    const data = await invoke<ProfileData>('load_profile', { name });
    setSelected(name);
    setSelectedData(data);
  };

  const handleApply = async () => {
    if (!selected || !selectedData) return;
    if (!window.confirm(t('profile.applyConfirm', { name: selected }))) return;
    useAppStore.getState().replaceBothPaths(
      selectedData.sys.map(e => e.path),
      selectedData.user.map(e => e.path),
    );
    // 同步 disabled 状态
    await invoke('save_disabled_state', {
      system: selectedData.sys.filter(e => !e.enabled).map(e => e.path),
      user: selectedData.user.filter(e => !e.enabled).map(e => e.path),
    });
    await useAppStore.getState().savePaths();
    onClose();
  };

  const handleDelete = async (name: string) => {
    if (!window.confirm(`删除配置文件 "${name}"？`)) return;
    await invoke('delete_profile', { name });
    if (selected === name) { setSelected(null); setSelectedData(null); }
    refreshProfiles();
  };

  const handleRename = async () => {
    if (!selected || !renameValue.trim()) return;
    await invoke('rename_profile', { oldName: selected, newName: renameValue.trim() });
    setRenameOpen(false);
    setSelected(renameValue.trim());
    refreshProfiles();
  };

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxHeight: '75vh' }}>
        <div className="flex items-center justify-between px-5 py-3 border-b" style={{ borderColor: 'var(--app-border)' }}>
          <h2 className="text-base font-semibold">{t('profile.title')}</h2>
          <div className="flex gap-2 items-center">
            <input
              type="text"
              value={newName}
              onChange={e => setNewName(e.target.value)}
              placeholder={t('profile.namePlaceholder')}
              className="px-2 py-1 text-sm rounded border outline-none w-44"
              style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)', borderColor: 'var(--app-border)' }}
            />
            <button
              className="px-3 py-1 text-sm rounded text-white"
              style={{ backgroundColor: '#3b82f6' }}
              disabled={saving || !newName.trim()}
              onClick={handleSave}
            >
              {t('profile.save')}
            </button>
            <button
              onClick={onClose}
              className="px-2 py-1 text-sm rounded hover:opacity-70 transition-opacity"
              style={{ color: 'var(--app-fg)' }}
              title="关闭"
            >
              ✕
            </button>
          </div>
        </div>

        <div className="flex flex-1 overflow-hidden">
          {/* 左侧：列表 */}
          <div className="w-48 border-r overflow-auto p-2" style={{ borderColor: 'var(--app-border)' }}>
            {profiles.length === 0 ? (
              <div className="text-xs text-center py-6" style={{ opacity: 0.5 }}>{t('profile.noProfiles')}</div>
            ) : (
              profiles.map(p => (
                <div
                  key={p.name}
                  onClick={() => handleLoad(p.name)}
                  className="px-2 py-1.5 text-sm rounded cursor-pointer mb-0.5"
                  style={{
                    backgroundColor: selected === p.name ? 'rgba(59,130,246,0.15)' : 'transparent',
                    color: selected === p.name ? '#3b82f6' : 'var(--app-fg)',
                  }}
                >
                  {p.name}
                </div>
              ))
            )}
          </div>

          {/* 右侧：详情 */}
          <div className="flex-1 p-3 overflow-auto">
            {!selectedData ? (
              <div className="text-center py-10 text-sm" style={{ opacity: 0.4 }}>
                {profiles.length === 0 ? t('profile.noProfiles') : '选择一个配置文件'}
              </div>
            ) : (
              <div>
                <div className="flex items-center gap-2 mb-3">
                  <span className="font-semibold text-sm">{selectedData.name}</span>
                  <span className="text-xs" style={{ opacity: 0.5 }}>{selectedData.modified}</span>
                </div>

                <div className="flex gap-1.5 mb-3">
                  <button
                    className="px-3 py-1 text-xs rounded text-white"
                    style={{ backgroundColor: '#3b82f6' }}
                    onClick={handleApply}
                  >
                    {t('profile.apply')}
                  </button>
                  <button
                    className="px-3 py-1 text-xs rounded"
                    style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)' }}
                    onClick={() => { setRenameOpen(true); setRenameValue(selectedData.name); }}
                  >
                    {t('profile.rename')}
                  </button>
                  <button
                    className="px-3 py-1 text-xs rounded text-white"
                    style={{ backgroundColor: '#ef4444' }}
                    onClick={() => handleDelete(selectedData.name)}
                  >
                    {t('profile.delete')}
                  </button>
                </div>

                {renameOpen && (
                  <div className="flex gap-2 mb-2">
                    <input
                      type="text"
                      value={renameValue}
                      onChange={e => setRenameValue(e.target.value)}
                      className="px-2 py-1 text-xs rounded border outline-none"
                      style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)', borderColor: 'var(--app-border)' }}
                    />
                    <button className="px-2 py-1 text-xs rounded text-white" style={{ backgroundColor: '#3b82f6' }} onClick={handleRename}>
                      确认
                    </button>
                  </div>
                )}

                <PathSection title={`系统 PATH (${selectedData.sys.length})`} paths={selectedData.sys} />
                <PathSection title={`用户 PATH (${selectedData.user.length})`} paths={selectedData.user} />
              </div>
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}

function PathSection({ title, paths }: { title: string; paths: PathEntry[] }) {
  return (
    <div className="mb-2">
      <div className="text-xs font-medium mb-1" style={{ opacity: 0.7 }}>{title}</div>
      {paths.length === 0 ? (
        <div className="text-xs" style={{ opacity: 0.4 }}>（空）</div>
      ) : (
        <div className="space-y-0.5 max-h-48 overflow-auto">
          {paths.map((e, i) => (
            <div
              key={i}
              className="text-xs font-mono px-2 py-0.5 rounded flex items-center gap-1.5"
              style={{
                backgroundColor: 'var(--app-list-bg)',
                color: e.enabled ? 'var(--app-fg)' : '#ef4444',
                textDecoration: e.enabled ? 'none' : 'line-through',
                opacity: e.enabled ? 1 : 0.5,
              }}
            >
              <span style={{ color: e.enabled ? '#22c55e' : '#ef4444', fontSize: 10 }}>
                {e.enabled ? '●' : '○'}
              </span>
              {e.path}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
