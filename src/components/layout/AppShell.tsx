import { useState, useEffect, useMemo } from 'react';
import { canWriteTarget, useAppStore, type TabId } from '@/store/app-store';
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
import { EnvVarTable } from '@/components/env-list/EnvVarTable';
import { EnvVarToolbar } from '@/components/env-list/EnvVarToolbar';
import { NewEnvVarDialog } from '@/components/dialogs/NewEnvVarDialog';
import { EditEnvVarDialog } from '@/components/dialogs/EditEnvVarDialog';
import { useEnvStore } from '@/store/env-store';
import { backend } from '@/services/backend';
import { envVarKey, findMetaByKey, type EnvVarMeta } from '@/core/env-var';

/** Tauri's File object includes the native filesystem path */
interface TauriFile extends File {
  path: string;
}

export function AppShell() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const setSelectedIndices = useAppStore((s) => s.setSelectedIndices);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const canWriteSystem = canWriteTarget(isAdmin, pathCapabilities, TargetType.SYSTEM);
  const canWriteUser = canWriteTarget(isAdmin, pathCapabilities, TargetType.USER);

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
  const [newVarOpen, setNewVarOpen] = useState(false);
  // 选中与编辑都只存稳定键；meta 一律从快照派生 —— 冲突刷新后重试自动
  // 携带新 revision（F-02），快照换代后悬空引用自动失效。
  const [selectedVarKey, setSelectedVarKey] = useState<string | null>(null);
  const [editVarKey, setEditVarKey] = useState<string | null>(null);
  const [envSearch, setEnvSearch] = useState('');
  const envSnapshot = useEnvStore((s) => s.snapshot);
  const loadEnvVars = useEnvStore((s) => s.load);

  const selectedVar = useMemo<EnvVarMeta | null>(() => {
    if (selectedVarKey === null || envSnapshot === null) return null;
    return findMetaByKey(envSnapshot, selectedVarKey);
  }, [selectedVarKey, envSnapshot]);

  // 进入「全部变量」时刷新列表与 revision；工具栏「刷新」共用同一入口。
  // 悬空选中无需在此清理：selectedVar 由快照派生，快照刷新后自动失效。
  useEffect(() => {
    if (activeTab === 'allVars') void loadEnvVars();
  }, [activeTab, loadEnvVars]);

  // Tauri 原生关窗确认（F-04）：X / Alt+F4 与工具栏「取消」走同一套检查 ——
  // PATH 有未保存修改或环境变量草稿未提交时先确认。草稿包含编辑弹窗中
  // 正在输入的实时内容（弹窗 onChange 镜像进 store）。非 Tauri 环境
  // （E2E mock、jsdom）没有原生窗口事件，静默跳过。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const handler = await getCurrentWindow().onCloseRequested((event) => {
          const pending = useAppStore.getState().isModified || useEnvStore.getState().hasDrafts();
          if (!pending) return; // 无待保存内容：不拦截，包装层自动 destroy
          event.preventDefault(); // 同步拦截默认销毁；确认后显式 destroy 收口（G-B2）
          void (async () => {
            try {
              const confirmed = await backend.confirmDialog(i18n.t('dialog.unsavedConfirm'));
              if (confirmed) await getCurrentWindow().destroy();
            } catch {
              // 对话框 IPC 失败时保守放行关窗：宁可丢草稿不可把窗口锁死
              await getCurrentWindow()
                .destroy()
                .catch(() => {});
            }
          })();
        });
        if (disposed) handler();
        else unlisten = handler;
      } catch {
        // 非 Tauri 运行环境（测试 / 浏览器预览），无原生关窗事件
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

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
    { id: 'allVars', label: t('tab.allVars') },
    { id: 'merged', label: t('tab.merged') },
  ];

  /** 确认后删除环境变量；成功后快照刷新，派生选中自动清除。 */
  const confirmRemoveVar = (meta: EnvVarMeta) => {
    // 异步确认替代阻塞式 window.confirm（同关窗路径机制）。删除是破坏性操作，
    // 对话框 IPC 失败时按「取消」处理（fail-closed），绝不静默删除。
    void backend
      .confirmDialog(t('envVar.deleteConfirm', { name: meta.name }))
      .then((confirmed) => {
        if (confirmed) void useEnvStore.getState().remove(meta);
      })
      .catch(() => {});
  };

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
        {/* 「全部变量」使用独立工具栏：PATH 的上移/下移/清理/导入/导出对其语义不成立。 */}
        {activeTab === 'allVars' ? (
          <EnvVarToolbar
            onCreate={() => setNewVarOpen(true)}
            onEdit={() => {
              if (selectedVar) setEditVarKey(selectedVarKey);
            }}
            onDelete={() => {
              if (selectedVar) confirmRemoveVar(selectedVar);
            }}
            onRefresh={() => void loadEnvVars()}
            onSearchChange={setEnvSearch}
            searchQuery={envSearch}
            selected={selectedVar}
          />
        ) : (
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
              // 环境变量草稿尚未提交时同样需要确认，否则关窗会静默丢弃用户输入。
              const hasPendingChanges = state.isModified || useEnvStore.getState().hasDrafts();
              if (!hasPendingChanges) {
                window.close();
                return;
              }
              void backend
                .confirmDialog(t('dialog.unsavedConfirm'))
                .then((confirmed) => {
                  if (confirmed) window.close();
                })
                .catch(() => {});
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
        )}
      </div>

      <div
        className="flex-1 overflow-auto"
        data-testid="path-drop-zone"
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'link';
        }}
        onDrop={(e) => {
          e.preventDefault();
          // 拖放只对 PATH 有语义：merged 与环境变量视图下直接忽略。
          if (activeTab === 'merged' || activeTab === 'allVars') return;
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
        ) : activeTab === 'allVars' ? (
          <EnvVarTable
            searchQuery={envSearch}
            onSelect={(meta) => setSelectedVarKey(envVarKey(meta))}
            onEdit={(meta) => {
              setSelectedVarKey(envVarKey(meta));
              setEditVarKey(envVarKey(meta));
            }}
            onDelete={(meta) => {
              setSelectedVarKey(envVarKey(meta));
              confirmRemoveVar(meta);
            }}
            onGoToPath={() => setActiveTab('system')}
            selectedKey={selectedVarKey}
          />
        ) : (
          <PathTable tabId={activeTab} />
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
        canWriteSystem={canWriteSystem}
        canWriteUser={canWriteUser}
        onSelect={actions.handleImportSelect}
        onCancel={() => setImportDialog({ open: false, system: [], user: [] })}
      />
      <AnalyzeDialog open={analyzeOpen} onClose={() => setAnalyzeOpen(false)} />
      <ProfileDialog open={profilesOpen} onClose={() => setProfilesOpen(false)} />
      {newVarOpen && (
        <NewEnvVarDialog
          canWriteSystem={canWriteSystem}
          canWriteUser={canWriteUser}
          onCancel={() => setNewVarOpen(false)}
          onConfirm={async (hive, name, value, kind) => {
            // Rust 是重复变量与权限的最终裁判；失败时弹窗保留并显示错误。
            const ok = await useEnvStore.getState().create(hive, name, value, kind);
            if (ok) setNewVarOpen(false);
            return ok;
          }}
        />
      )}
      {editVarKey && (
        <EditEnvVarDialog
          varKey={editVarKey}
          onCancel={() => setEditVarKey(null)}
          onConfirm={async (value, readRevision) => {
            const store = useEnvStore.getState();
            // 从最新快照派生 meta：冲突刷新后重试自动携带新 revision（F-02）
            const meta = store.snapshot ? findMetaByKey(store.snapshot, editVarKey) : null;
            if (!meta) return false;
            store.setDraft(meta, value);
            const ok = await store.save(meta, readRevision);
            if (ok) setEditVarKey(null);
            // 失败不清草稿（c2 统一策略）：草稿镜像输入，供重试与关窗确认。
            return ok;
          }}
        />
      )}
    </div>
  );
}
