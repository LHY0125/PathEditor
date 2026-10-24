import { invoke } from '@tauri-apps/api/core';
import {
  confirm as tauriConfirm,
  open as tauriOpen,
  save as tauriSave,
} from '@tauri-apps/plugin-dialog';
import type { PathEntry } from '@/core/path-entry';
import type { EnvHive, EnvValueKind } from '@/core/env-var';

export type { PathCapabilities } from '@/core/path-capabilities';
export type { ConflictEntry, ConflictLocation, ScanResult, ToolGroup } from './parse-core';

export interface ProfileMeta {
  name: string;
  created: string;
  modified: string;
}

export interface ProfileData {
  name: string;
  sys: PathEntry[];
  user: PathEntry[];
  created: string;
  modified: string;
}

export * from './parse-core';
export * from './parse-data';

import {
  parseApplyOutcome,
  parseBackupPath,
  parseCoreError,
  parseEnvBackupInfo,
  parseEnvVarSnapshot,
  isRecord,
  parsePathCapabilities,
  parsePathEntries,
  parsePathSnapshot,
  parseRevealedValue,
  parseRestoreOutcome,
  parseRestorePreview,
  parseScanResult,
  parseWriteOutcome,
} from './parse-core';
import type { PathEntryWrite } from './parse-core';
import type { EventFilter, ProfileView } from './parse-data';
import {
  parseCompareResult,
  parseCycleList,
  parseDashboard,
  parseEventPage,
  parseProfileList,
  parseRefEdges,
  parseSearchHits,
  parseSnapshotView,
  parseSyncConflicts,
  parseSyncExportReport,
  parseSyncImportReport,
  parseSyncStatus,
} from './parse-data';

/** 唯一的 Tauri IPC 边界；组件和 Store 不再直接拼命令字符串。 */
export const backend = {
  loadPathSnapshot: async () =>
    parsePathSnapshot(await parseCoreError(invoke<unknown>('load_path_snapshot'))),
  /**
   * 唯一的「保存 PATH + sidecar」编排入口。
   *
   * core 侧负责：写前备份、乐观并发校验（`expected`）、顺序写（system→user）、
   * 广播、sidecar/pending 事务。GUI 只传数据并映射结果。
   *
   * @returns 经 `parseApplyOutcome` 运行时校验的 `ApplyOutcome`。
   */
  savePathWithSidecar: async (request: {
    system: PathEntryWrite | null;
    user: PathEntryWrite | null;
  }) => parseApplyOutcome(await parseCoreError(invoke<unknown>('save_path_with_sidecar', request))),
  checkAdmin: () => invoke<boolean>('check_admin'),
  getPathCapabilities: async () =>
    parsePathCapabilities(await invoke<unknown>('get_path_capabilities')),
  validatePath: (path: string) => invoke<boolean>('validate_path', { path }),
  expandEnvVars: (path: string) => invoke<string>('expand_env_vars', { path }),
  /** 异步确认对话框（替代阻塞式 window.confirm）：关窗确认等路径专用，绝不阻塞 JS 线程。 */
  confirmDialog: (message: string) => tauriConfirm(message),
  /** 选择同步包导出位置（原生保存对话框）；用户取消返回 null。 */
  pickSyncExportPath: (filterName: string) =>
    tauriSave({
      defaultPath: 'patheditor-sync.pesync',
      filters: [{ name: filterName, extensions: ['pesync'] }],
    }),
  /** 选择要导入的同步包（原生打开对话框）；用户取消返回 null。 */
  pickSyncImportPath: async (filterName: string) => {
    const selected = await tauriOpen({
      multiple: false,
      filters: [{ name: filterName, extensions: ['pesync'] }],
    });
    return typeof selected === 'string' ? selected : null;
  },
  importFile: (path: string) => invoke<[PathEntry[], PathEntry[]]>('import_file', { path }),
  exportPathEntries: (sys: PathEntry[], usr: PathEntry[], format: string) =>
    invoke<string>('export_path_entries', { sys, usr, format }),
  cleanPathEntries: (entries: PathEntry[]) =>
    invoke<[PathEntry[], PathEntry[]]>('clean_path_entries', { entries }),
  scanPaths: async (paths: string[], query = '') =>
    parseScanResult(await parseCoreError(invoke<unknown>('scan_paths', { paths, query }))),
  /**
   * 配置方案统一走 DB。
   */
  profileList: async () =>
    parseProfileViewList(await parseCoreError(invoke<unknown>('profile_list')), 'profile_list'),
  profileSave: async (request: ProfileSaveRequest) => {
    const result = await parseCoreError(invoke<unknown>('profile_save', { request }));
    if (typeof result !== 'string' || result.length === 0) {
      throw new Error('profile_save 必须返回非空 uid');
    }
    return result;
  },
  profilePaths: async (name: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_paths', { name }));
    return result === null ? null : parseProfilePathsView(result, 'profile_paths');
  },
  profileRename: async (oldName: string, newName: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_rename', { oldName, newName }));
    if (typeof result !== 'boolean') throw new Error('profile_rename 必须返回布尔值');
    return result;
  },
  profileDeleteByName: async (name: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_delete_by_name', { name }));
    if (typeof result !== 'boolean') throw new Error('profile_delete_by_name 必须返回布尔值');
    return result;
  },
  /**
   * 应用配置方案：core 读配置 → 写注册表与当前快照 → 记录应用次数。
   *
   * 与 CLI 的 `profile apply` 共用同一编排；记账失败不影响保存结果。
   */
  profileApply: async (name: string) =>
    parseApplyOutcome(await parseCoreError(invoke<unknown>('profile_apply', { name }))),
  listAllEnvVars: async () => parseEnvVarSnapshot(await invoke<unknown>('list_all_env_vars')),
  revealEnvVar: async (hive: EnvHive, name: string) =>
    parseRevealedValue(
      await parseCoreError(invoke<unknown>('reveal_env_var', { hive, name })),
      'reveal_env_var',
    ),
  /**
   * 写入已有变量。成功返回 `WriteOutcome`；rejection 经 `parseCoreError` 结构化。
   *
   * 注意 `parseCoreError` 的返回值类型是 `unknown` —— 形状由
   * `parseWriteOutcome` 收口，不靠 TS 断言。
   */
  updateEnvVar: async (hive: EnvHive, name: string, value: string, expectedRevision: string) =>
    parseWriteOutcome(
      await parseCoreError(
        invoke<unknown>('update_env_var', { hive, name, value, expectedRevision }),
      ),
      'update_env_var',
    ),
  createEnvVar: async (hive: EnvHive, name: string, value: string, kind: EnvValueKind) =>
    parseWriteOutcome(
      await parseCoreError(invoke<unknown>('create_env_var', { hive, name, value, kind })),
      'create_env_var',
    ),
  deleteEnvVar: async (hive: EnvHive, name: string, expectedRevision: string) =>
    parseWriteOutcome(
      await parseCoreError(invoke<unknown>('delete_env_var', { hive, name, expectedRevision })),
      'delete_env_var',
    ),
  /** 立即创建一份环境变量备份，返回备份文件路径。 */
  backupEnvVars: async () =>
    parseBackupPath(await parseCoreError(invoke<unknown>('backup_env_vars')), 'backup_env_vars'),
  /** 列出已有的环境变量备份（Rust 侧按时间倒序，不解析内容）。 */
  listEnvBackups: async () => {
    const label = 'list_env_backups';
    const result = await parseCoreError(invoke<unknown>(label));
    if (!Array.isArray(result)) throw new Error(`${label} 返回了无效的 EnvBackupInfo[] 契约`);
    return result.map((item) => parseEnvBackupInfo(item, label));
  },
  /**
   * 计算备份相对当前注册表的差异（不写任何内容）。
   *
   * 路径来源校验（扩展名 / 是否在备份目录内 / 大小上限）**在本命令内于 Rust 侧
   * 先于读取执行**：Rust 的 `preview_restore_file` 自身不校验，对不可解析的文件
   * 会经 persist 层把它重命名隔离为 `<file>.corrupt-<ts>`。Tauri 命令
   * `preview_env_backup` 的第一行即 `validate_backup_path`，前端不复制该规则。
   */
  previewEnvBackup: async (file: string) =>
    parseRestorePreview(await parseCoreError(invoke<unknown>('preview_env_backup', { file }))),
  /**
   * 执行恢复。`force = false` 时遇冲突整批中止并返回 code=`conflict`。
   *
   * `force = true` 会覆盖冲突项，但那些变量仍计入 `preview.conflicts` ——
   * 确认弹窗因此**低报**改动量，这是 Rust 侧的已知取舍，前端不做换算。
   */
  restoreEnvBackup: async (file: string, force: boolean) =>
    parseRestoreOutcome(
      await parseCoreError(invoke<unknown>('restore_env_backup', { file, force })),
    ),

  // ── 历史 ────────────

  /** 列出历史事件（分页，服务端固定时间倒序）。 */
  historyListEvents: async (filter?: EventFilter) =>
    parseEventPage(await parseCoreError(invoke<unknown>('history_list_events', { filter }))),
  /**
   * 读取快照预览。
   *
   * `reveal = false`（默认）时敏感变量的 `value` 恒为 `null` —— 由 **Rust 侧**掩码，
   * 前端不做「拿到明文再隐藏」的动作。需要明文必须显式传 `reveal: true`。
   */
  historyGetSnapshot: async (uid: string, reveal = false) =>
    parseSnapshotView(
      await parseCoreError(invoke<unknown>('history_get_snapshot', { uid, reveal })),
      'history_get_snapshot',
    ),
  /**
   * 把历史快照恢复回注册表。
   *
   * **破坏性操作**：会覆盖当前 PATH。UI 必须先二次确认。
   * 恢复走既有注册表编排（含写前备份），成功后追加新快照与新事件（append-only）。
   */
  historyRestoreSnapshot: async (uid: string) =>
    parseApplyOutcome(await parseCoreError(invoke<unknown>('history_restore_snapshot', { uid }))),
  /** 读取当前快照；无当前快照或降级模式下返回 `null`。 */
  historyGetCurrent: async () => {
    const result = await parseCoreError(invoke<unknown>('history_get_current'));
    return result === null ? null : parseSnapshotView(result, 'history_get_current');
  },
  /**
   * 显式 reveal 一个历史快照里的环境变量值。
   *
   * 与 `historyGetSnapshot(uid, true)` 的区别：后者是**批量**揭示，
   * 前者是**单个变量**的显式请求（UI 上每个被掩码变量旁的「显示」按钮）。
   *
   * **调用方不得把返回值写入 store** —— 明文进入持久状态会泄漏到
   * DevTools / 状态快照。只在该次渲染的局部 state 里持有。
   *
   * `restorable = 0` 的变量（`Unsupported` 类型）没有值，Rust 侧返回
   * `NotFound`，前端应显示「此类型不支持查看」而不是空串。
   */
  historyRevealEnvValue: async (uid: string, hive: string, name: string): Promise<string> => {
    const result = await parseCoreError(
      invoke<unknown>('history_reveal_env_value', { uid, hive, name }),
    );
    if (typeof result !== 'string') {
      throw new Error('history_reveal_env_value 必须返回字符串');
    }
    return result;
  },
  /** 软删除配置方案（按 uid）；返回是否真的删除了（已删除返回 false）。 */
  /* ────────────────────── 检索 / 统计 / 对比 / 引用图 ────────────────────── */

  /**
   * FTS5 全文检索。
   *
   * **空查询由后端直接返回空数组**（见 `core::db::search::search`），
   * 前端不应依赖这一点来省请求——搜索框为空时 UI 也不该发起调用
   * ，两层各守一次。
   *
   * 查询串由 Rust 侧做 FTS5 转义，前端传原始用户输入即可。
   */
  search: async (query: string, limit = 50) =>
    parseSearchHits(await parseCoreError(invoke<unknown>('search', { query, limit })), 'search'),

  /** 统计面板数据；`days` 缺省 30。 */
  statsDashboard: async (days = 30) =>
    parseDashboard(
      await parseCoreError(invoke<unknown>('stats_dashboard', { days })),
      'stats_dashboard',
    ),

  /** 比较两个快照（system 与 user 分开返回）。 */
  compareSnapshots: async (leftUid: string, rightUid: string) =>
    parseCompareResult(
      await parseCoreError(invoke<unknown>('compare_snapshots', { leftUid, rightUid })),
      'compare_snapshots',
    ),

  /** 谁引用了 `name`（反向边）。 */
  refsDependents: async (name: string) =>
    parseRefEdges(
      await parseCoreError(invoke<unknown>('refs_dependents', { name })),
      'refs_dependents',
    ),

  /** `name` 传递依赖了谁（递归）。 */
  refsDependencies: async (name: string) =>
    parseRefEdges(
      await parseCoreError(invoke<unknown>('refs_dependencies', { name })),
      'refs_dependencies',
    ),

  /** 引用环列表。 */
  refsCycles: async () =>
    parseCycleList(await parseCoreError(invoke<unknown>('refs_cycles')), 'refs_cycles'),

  /* ────────────────────── 同步中心 ────────────────────── */

  /**
   * 同步状态：待导出变更数 + 未解决冲突数。
   *
   * 降级模式（DB 不可用）会以 `database` 码拒绝，调用方据此显示不可用。
   */
  syncStatus: async () =>
    parseSyncStatus(await parseCoreError(invoke<unknown>('sync_status')), 'sync_status'),

  /**
   * 导出同步包。
   *
   * `passphrase` 只在本调用内使用：**不写入 store、不记录日志、不回显**。
   * 组件侧用 `type="password"` + 局部 state 持有它（与历史快照 reveal 同口径）。
   */
  syncExport: async (passphrase: string, path: string) =>
    parseSyncExportReport(
      await parseCoreError(invoke<unknown>('sync_export', { passphrase, path })),
      'sync_export',
    ),

  /** 导入同步包。**只改 DB，不写注册表**。 */
  syncImport: async (passphrase: string, path: string) =>
    parseSyncImportReport(
      await parseCoreError(invoke<unknown>('sync_import', { passphrase, path })),
      'sync_import',
    ),

  /** 冲突列表；`includeResolved = true` 时含已解决的。 */
  syncListConflicts: async (includeResolved = false) =>
    parseSyncConflicts(
      await parseCoreError(invoke<unknown>('sync_list_conflicts', { includeResolved })),
      'sync_list_conflicts',
    ),

  /** 解决一条冲突。`resolution` 取 `local` / `remote`。 */
  syncResolveConflict: async (id: number, resolution: 'local' | 'remote') => {
    const result = await parseCoreError(
      invoke<unknown>('sync_resolve_conflict', { id, resolution }),
    );
    if (result !== null && result !== undefined) {
      throw new Error('sync_resolve_conflict 不应返回数据');
    }
  },
};

// ── 配置方案（DB 版） ────────────────────────

/** 保存配置的入参（对应 Rust 的 `ProfileSaveRequest`）。 */
export interface ProfileSaveRequest {
  name: string;
  description: string | null;
  tags: string[];
  system: PathEntry[];
  user: PathEntry[];
}

/** 配置的 PATH 条目（对应 Rust 的 `ProfilePathsView`）。 */
export interface ProfilePathsView {
  name: string;
  /** 最后更新时间（Unix 毫秒） */
  updatedAt: number;
  system: PathEntry[];
  user: PathEntry[];
}

/** 校验 `profile_list` 的返回：`ProfileView[]`（见上面 `ProfileView` 接口）。 */
function parseProfileViewList(value: unknown, label: string): ProfileView[] {
  return parseProfileList(value, label);
}

/** 校验 `profile_paths` 的返回。 */
function parseProfilePathsView(value: unknown, label: string): ProfilePathsView {
  if (!isRecord(value) || typeof value.name !== 'string' || typeof value.updatedAt !== 'number') {
    throw new Error(`${label} 返回了无效的配置路径契约`);
  }
  return {
    name: value.name,
    updatedAt: value.updatedAt,
    system: parsePathEntries(value.system, `${label}.system`),
    user: parsePathEntries(value.user, `${label}.user`),
  };
}
