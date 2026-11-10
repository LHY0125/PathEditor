import { useState, useEffect, useMemo } from 'react';
import { canWriteTarget, useAppStore, type TabId } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { TargetType } from '@/core/undo-redo';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { StatusBar } from './StatusBar';
import { TitleBar } from './TitleBar';
import { DegradedBanner } from './DegradedBanner';
import { ToolBar } from '@/components/toolbar/ToolBar';
import { Button } from '@/components/ui/Button';
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
import { EnvBackupDialog } from '@/components/dialogs/EnvBackupDialog';
import { RulesDialog } from '@/components/dialogs/RulesDialog';
import { HistoryPanel } from '@/components/history/HistoryPanel';
import { SearchPanel } from '@/components/search/SearchPanel';
import { StatsPanel } from '@/components/stats/StatsPanel';
import { SyncCenter } from '@/components/sync/SyncCenter';
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
  const [envBackupOpen, setEnvBackupOpen] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);
  // 选中与编辑都只存稳定键；meta 一律从快照派生 —— 冲突刷新后重试自动
  // 携带新 revision，快照换代后悬空引用自动失效。
  // 选中键放 store（而不是局部 state）：搜索结果需要跨面板定位到变量。
  const selectedVarKey = useEnvStore((s) => s.selectedKey);
  const setSelectedVarKey = useEnvStore((s) => s.setSelectedKey);
  const [editVarKey, setEditVarKey] = useState<string | null>(null);
  const [envSearch, setEnvSearch] = useState('');
  const envSnapshot = useEnvStore((s) => s.snapshot);
  const loadEnvVars = useEnvStore((s) => s.load);

  const selectedVar = useMemo<EnvVarMeta | null>(() => {
    if (selectedVarKey === null || envSnapshot === null) return null;
    return findMetaByKey(envSnapshot, selectedVarKey);
  }, [selectedVarKey, envSnapshot]);

  // 原生拖放：Tauri 2 在 Windows 上拦截文件拖放，DOM 的 `drop` 收不到事件，
  // `file.path` 也只在 Tauri v1 存在。必须用 `onDragDropEvent` 取原生路径。
  // 下面的 DOM handler 仅保留给浏览器 / E2E（mock IPC）环境。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        unlisten = await getCurrentWindow().onDragDropEvent((event) => {
          if (event.payload.type !== 'drop') return;
          // 先把 paths 取到局部常量：TS 不会把 `event.payload` 的收窄带进下面的闭包
          const droppedPaths = event.payload.paths;
          const tab = useAppStore.getState().activeTab;
          if (tab !== 'system' && tab !== 'user') return;
          const target = tab === 'user' ? TargetType.USER : TargetType.SYSTEM;
          // 写权限必须在入口拦掉：非管理员拖到「系统 PATH」时若仍加进草稿，
          // 保存按钮是禁用的，撤销也因「无历史」不可用 —— 用户会卡在一个
          // 既保存不了、又清不掉的「未保存」状态里。
          const { isAdmin, pathCapabilities, setStatusError } = useAppStore.getState();
          if (!canWriteTarget(isAdmin, pathCapabilities, target)) {
            setStatusError(
              i18n.t(
                target === TargetType.USER
                  ? 'status.noUserPermission'
                  : 'status.noSystemPermission',
              ),
            );
            return;
          }
          void (async () => {
            let rejected = 0;
            for (const droppedPath of droppedPaths) {
              // PATH 条目必须是目录；含 `%VAR%` 的路径由 core 判为未知并放行。
              try {
                if (await backend.validatePath(droppedPath)) {
                  useAppStore.getState().addPath(droppedPath, target);
                } else {
                  rejected += 1;
                }
              } catch {
                // 校验失败按「无法判断」处理：不添加，也不误报「只能拖拽文件夹」
              }
            }
            // 拖入的是文件时给出明确提示，而不是静默丢弃
            if (rejected > 0) {
              useAppStore.getState().setStatusMessage(i18n.t('status.dragFolderOnly'));
            }
          })();
        });
        if (disposed) unlisten();
      } catch {
        // 非 Tauri 运行环境（测试 / 浏览器预览）：没有原生拖放事件
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // `<html lang>` 必须跟随语言切换：读屏按它选发音。
  const syncHtmlLang = () => {
    const current = i18n.resolvedLanguage ?? i18n.language;
    document.documentElement.lang = current.startsWith('zh') ? 'zh-CN' : 'en';
  };
  useEffect(() => {
    syncHtmlLang();
    // 只挂在挂载时不够：语言也可能由其它入口改变（快捷键、系统事件），
    // 读屏用的 `lang` 必须跟着 i18next 的真实状态走。
    i18n.on('languageChanged', syncHtmlLang);
    return () => i18n.off('languageChanged', syncHtmlLang);
  }, []);

  // 进入「全部变量」时刷新列表与 revision；工具栏「刷新」共用同一入口。
  // 悬空选中无需在此清理：selectedVar 由快照派生，快照刷新后自动失效。
  useEffect(() => {
    if (activeTab === 'allVars') void loadEnvVars();
  }, [activeTab, loadEnvVars]);

  // Tauri 原生关窗确认：X / Alt+F4 与工具栏「取消」走同一套检查 ——
  // PATH 有未保存修改或环境变量草稿未提交时先确认。草稿包含编辑弹窗中
  // 正在输入的实时内容（弹窗 onChange 镜像进 store）。非 Tauri 环境
  // （E2E mock、jsdom）没有原生窗口事件，静默跳过。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const handler = await getCurrentWindow().onCloseRequested((event) => {
          const pending = useAppStore.getState().isModified || useEnvStore.getState().hasDrafts();
          if (!pending) return; // 无待保存内容：不拦截，包装层自动 destroy
          event.preventDefault(); // 同步拦截默认销毁；确认后显式 destroy 收口
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
    { id: 'history', label: t('tab.history') },
    { id: 'search', label: t('tab.search') },
    { id: 'stats', label: t('tab.stats') },
    { id: 'sync', label: t('tab.sync') },
  ];

  /**
   * 确认后删除环境变量；成功后快照刷新，派生选中自动清除。
   *
   * 确认文案带**引用数**：被其它变量 `%NAME%` 引用的条目删掉后会让那些引用
   * 失效（core 只校验格式，不会替你改引用方）。查询失败不阻断删除流程 ——
   * 拿不到引用信息时退回普通文案，而不是把删除入口整个卡死。
   */
  const confirmRemoveVar = (meta: EnvVarMeta) => {
    void (async () => {
      let dependents: number | null;
      try {
        dependents = (await backend.refsDependents(meta.name)).length;
      } catch {
        dependents = null;
      }
      const message =
        dependents !== null && dependents > 0
          ? t('envVar.deleteConfirmWithRefs', { name: meta.name, count: dependents })
          : t('envVar.deleteConfirm', { name: meta.name });
      try {
        // 异步确认替代阻塞式 window.confirm。删除是破坏性操作，
        // 对话框 IPC 失败时按「取消」处理（fail-closed），绝不静默删除。
        if (await backend.confirmDialog(message)) {
          void useEnvStore.getState().remove(meta);
        }
      } catch {
        // fail-closed：确认框没弹出来就等于用户没同意
      }
    })();
  };

  return (
    <div className="app-root">
      <TitleBar />
      {/* 降级条幅必须常驻，与当前标签页无关 */}
      <DegradedBanner />

      <nav className="tabs" role="tablist" aria-label={t('app.name')}>
        {tabConfig.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={`tab-${tab.id}`}
            aria-selected={activeTab === tab.id}
            aria-controls={activeTab === tab.id ? 'tabpanel' : undefined}
            tabIndex={activeTab === tab.id ? 0 : -1}
            data-active={activeTab === tab.id}
            onClick={() => {
              setActiveTab(tab.id);
              setSelectedIndices([]);
            }}
            onKeyDown={(e) => {
              const index = tabConfig.findIndex((item) => item.id === tab.id);
              let nextIndex: number | null = null;
              if (e.key === 'ArrowRight') nextIndex = (index + 1) % tabConfig.length;
              else if (e.key === 'ArrowLeft') {
                nextIndex = (index - 1 + tabConfig.length) % tabConfig.length;
              } else if (e.key === 'Home') nextIndex = 0;
              else if (e.key === 'End') nextIndex = tabConfig.length - 1;
              if (nextIndex === null) return;
              e.preventDefault();
              const next = tabConfig[nextIndex];
              setActiveTab(next.id);
              setSelectedIndices([]);
              document.getElementById(`tab-${next.id}`)?.focus();
            }}
            className="tab-pill"
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {/* 各 Tab 的工具栏语义不同：PATH 编辑动作只属于 system/user；
          merged 只读；其余面板自带标题/筛选/操作区，不能再显示 PATH 工具栏。 */}
      {activeTab === 'allVars' && (
        <EnvVarToolbar
          onCreate={() => setNewVarOpen(true)}
          onEdit={() => {
            if (selectedVar) setEditVarKey(selectedVarKey);
          }}
          onDelete={() => {
            if (selectedVar) confirmRemoveVar(selectedVar);
          }}
          onRefresh={() => void loadEnvVars()}
          onBackup={() => setEnvBackupOpen(true)}
          onSearchChange={setEnvSearch}
          searchQuery={envSearch}
          selected={selectedVar}
        />
      )}
      {(activeTab === 'system' || activeTab === 'user') && (
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
            // 以 i18next 解析后的语言为准，不能读 localStorage 原始值：
            // 检测器会缓存 'en-US' 这类区域变体，与 'zh-CN' 比较会得出错误结论。
            const current = i18n.resolvedLanguage ?? i18n.language;
            // 不在这里手调 syncHtmlLang：changeLanguage 会触发 languageChanged，
            // 由上面的监听统一更新，避免两条路径各写一份。
            void i18n.changeLanguage(current.startsWith('zh') ? 'en' : 'zh-CN');
          }}
          onProfiles={() => setProfilesOpen(true)}
          onRules={() => setRulesOpen(true)}
          onAnalyze={() => setAnalyzeOpen(true)}
          onDarkMode={() => useThemeStore.getState().toggle()}
          onSystemTheme={() => useThemeStore.getState().setMode('system')}
        />
      )}
      {activeTab === 'merged' && (
        <div className="merge-hint">
          <span>{t('merge.readOnly')}</span>
          <span className="merge-hint-spacer" />
          <Button size="sm" variant="ghost" onClick={() => setActiveTab('system')}>
            {t('merge.gotoSystem')}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setActiveTab('user')}>
            {t('merge.gotoUser')}
          </Button>
        </div>
      )}

      <div
        id="tabpanel"
        role="tabpanel"
        aria-labelledby={`tab-${activeTab}`}
        className="flex-1 min-h-0 flex flex-col overflow-hidden"
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
        ) : activeTab === 'history' ? (
          <HistoryPanel />
        ) : activeTab === 'search' ? (
          <SearchPanel />
        ) : activeTab === 'stats' ? (
          <StatsPanel />
        ) : activeTab === 'sync' ? (
          <SyncCenter />
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
      {/* 恢复改的是注册表：成功后刷新环境变量表格，否则界面与实际不一致。 */}
      <RulesDialog open={rulesOpen} onClose={() => setRulesOpen(false)} />
      <EnvBackupDialog
        open={envBackupOpen}
        onClose={() => setEnvBackupOpen(false)}
        onRestored={() => void loadEnvVars()}
      />
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
            // 从最新快照派生 meta：冲突刷新后重试自动携带新 revision
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
