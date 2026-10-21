import { invoke } from '@tauri-apps/api/core';
import { confirm as tauriConfirm } from '@tauri-apps/plugin-dialog';
import type { PathEntry, PathSnapshot } from '@/core/path-entry';
import type { PathCapabilities } from '@/core/path-capabilities';
import type {
  CoreError,
  EnvHive,
  EnvValueKind,
  EnvVarMeta,
  EnvVarSnapshot,
  ErrorCode,
  RevealedValue,
} from '@/core/env-var';
import type { BackupOutcome, WriteOutcome } from '@/core/env-var';
import type {
  EnvBackupInfo,
  RestoreChange,
  RestoreChangeKind,
  RestoreOutcome,
  RestorePreview,
} from '@/core/env-backup';

export type { PathCapabilities } from '@/core/path-capabilities';

export interface ConflictLocation {
  dir: string;
  priority: number;
}

export interface ConflictEntry {
  name: string;
  locations: ConflictLocation[];
}

export interface ToolGroup {
  dir: string;
  exists: boolean;
  exes: string[];
}

export interface ScanResult {
  conflicts: ConflictEntry[];
  tools: ToolGroup[];
}

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isPathEntry(value: unknown): value is PathEntry {
  return isRecord(value) && typeof value.path === 'string' && typeof value.enabled === 'boolean';
}

function parsePathEntries(value: unknown, label: string): PathEntry[] {
  if (!Array.isArray(value) || !value.every(isPathEntry)) {
    throw new Error(`${label} 返回了无效的 PathEntry[] 契约`);
  }
  return value.map((entry) => ({ path: entry.path, enabled: entry.enabled }));
}

function parseStringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || !value.every((item) => typeof item === 'string')) {
    throw new Error(`${label} 返回了无效的字符串数组契约`);
  }
  return value;
}

function parsePathSnapshot(value: unknown): PathSnapshot {
  if (!isRecord(value)) throw new Error('load_path_snapshot 返回了无效的快照契约');
  return {
    system: parsePathEntries(value.system, 'load_path_snapshot.system'),
    user: parsePathEntries(value.user, 'load_path_snapshot.user'),
  };
}

/**
 * 单个 hive 的写入请求（对应 core 的 `PathHiveWrite`）。
 *
 * - `entries`：待写入的完整条目（含 `enabled`）；
 * - `expected`：乐观并发的期望当前值（**启用**路径）；`null` = 跳过校验。
 */
export interface PathEntryWrite {
  entries: PathEntry[];
  expected: string[] | null;
}

/** 单个 hive 的应用结果（对应 core 的 `HiveOutcome`）。 */
export type HiveOutcome =
  | 'applied'
  | 'skipped'
  | { failed: { code: ErrorCode; operation: string; message: string } };

/** sidecar 快照的落盘结果（对应 core 的 `SidecarOutcome`）。 */
export type SidecarOutcome =
  | 'saved'
  | { pending: { code: ErrorCode; operation: string; message: string } }
  | { failed: { code: ErrorCode; operation: string; message: string } };

/** 一次 PATH 应用（注册表 + sidecar）的整体结果（对应 core 的 `ApplyOutcome`）。 */
export interface ApplyOutcome {
  system: HiveOutcome;
  user: HiveOutcome;
  sidecar: SidecarOutcome;
}

/** 校验单个 hive 结果（`'applied' | 'skipped' | { failed }`）。 */
function parseHiveOutcome(value: unknown, label: string): HiveOutcome {
  if (value === 'applied' || value === 'skipped') return value;
  if (isRecord(value) && 'failed' in value) {
    const f = value.failed;
    if (!isRecord(f) || typeof f.code !== 'string' || typeof f.message !== 'string') {
      throw new Error(`${label} 的 failed 形状无效`);
    }
    return {
      failed: {
        code: f.code as ErrorCode,
        operation: typeof f.operation === 'string' ? f.operation : '',
        message: f.message,
      },
    };
  }
  throw new Error(`${label} 返回了无效的 hive 结果契约`);
}

/** 校验 sidecar 结果（`'saved' | { pending } | { failed }`）。 */
function parseSidecarOutcome(value: unknown): SidecarOutcome {
  if (value === 'saved') return value;
  if (isRecord(value) && 'pending' in value) {
    const p = value.pending;
    if (!isRecord(p) || typeof p.code !== 'string' || typeof p.message !== 'string') {
      throw new Error('sidecar.pending 形状无效');
    }
    return {
      pending: {
        code: p.code as ErrorCode,
        operation: typeof p.operation === 'string' ? p.operation : '',
        message: p.message,
      },
    };
  }
  if (isRecord(value) && 'failed' in value) {
    const f = value.failed;
    if (!isRecord(f) || typeof f.code !== 'string' || typeof f.message !== 'string') {
      throw new Error('sidecar.failed 形状无效');
    }
    return {
      failed: {
        code: f.code as ErrorCode,
        operation: typeof f.operation === 'string' ? f.operation : '',
        message: f.message,
      },
    };
  }
  throw new Error('save_path_with_sidecar 返回了无效的 sidecar 契约');
}

/** 运行时校验 `save_path_with_sidecar` 的完整返回形状。 */
function parseApplyOutcome(value: unknown): ApplyOutcome {
  if (!isRecord(value)) throw new Error('save_path_with_sidecar 返回了无效的跨层契约');
  return {
    system: parseHiveOutcome(value.system, 'save_path_with_sidecar.system'),
    user: parseHiveOutcome(value.user, 'save_path_with_sidecar.user'),
    sidecar: parseSidecarOutcome(value.sidecar),
  };
}

function parsePathCapabilities(value: unknown): PathCapabilities {
  if (
    !isRecord(value) ||
    typeof value.canReadSystem !== 'boolean' ||
    typeof value.canWriteSystem !== 'boolean' ||
    typeof value.canReadUser !== 'boolean' ||
    typeof value.canWriteUser !== 'boolean'
  ) {
    throw new Error('get_path_capabilities 返回了无效的能力契约');
  }
  return {
    canReadSystem: value.canReadSystem,
    canWriteSystem: value.canWriteSystem,
    canReadUser: value.canReadUser,
    canWriteUser: value.canWriteUser,
  };
}

const ENV_VALUE_KINDS: readonly EnvValueKind[] = ['string', 'expandString', 'unsupported'];
const ENV_HIVES: readonly EnvHive[] = ['system', 'user'];

/** 合法 ErrorCode 字符串白名单（与 Rust `ErrorCode` serde camelCase 一致）。 */
const ERROR_CODES: readonly ErrorCode[] = [
  'conflict',
  'reservedName',
  'protected',
  'unsupportedType',
  'permissionDenied',
  'notFound',
  'nameExists',
  'invalidName',
  'invalidValue',
  'io',
  'parse',
  // v6：数据库/密钥/同步/迁移。降级模式（DB 不可用）的判定依赖这几个码，
  // 漏加会让 parseCoreError 静默兜底成 internal，前端再也识别不出降级。
  'database',
  'crypto',
  'sync',
  'syncFormat',
  'migration',
  'internal',
];

/**
 * rejection 的保证形状：`code` 与 `message` 恒存在且为 string。
 * 完整 CoreError 的其余字段（operation/hive/name/retryable）属透传信息，
 * 不在解析层保证 —— 判定与展示只依赖这两个字段。
 */
type RejectionPayload = Pick<CoreError, 'code' | 'message'>;

/**
 * 把 Tauri invocation 的 rejection 解析为结构化 `CoreError`（F-06）。
 *
 * 双形状兼容（过渡期，Wave 2 收口时复核是否收紧）：
 * - env 命令的 Rust 错误已迁移为 `CoreError`，Tauri rejection 是
 *   `{code, message, ...}` 对象 → 白名单校验 `code` 后结构化透传；
 * - PATH 命令仍返回纯文本 `String`（后续波次迁移）→ 兜底为
 *   `{code:'internal', message: 原文}`。
 *
 * 白名单构造：对象但 `code` 不是合法 ErrorCode、或 `message` 缺失时，
 * 一律降级为 `internal`，绝不透传来路不明的字段。
 */
export async function parseCoreError(promise: Promise<unknown>): Promise<unknown> {
  try {
    return await promise;
  } catch (error: unknown) {
    if (
      isRecord(error) &&
      typeof error.code === 'string' &&
      ERROR_CODES.includes(error.code as ErrorCode) &&
      typeof error.message === 'string'
    ) {
      const payload: RejectionPayload = { code: error.code as ErrorCode, message: error.message };
      throw payload;
    }
    // 未知形状兜底：对象且带非空 message 时保留原文（String(error) 会丢成
    // "[object Object]"），否则退回字符串化结果。
    const fallbackMessage =
      isRecord(error) && typeof error.message === 'string' && error.message.length > 0
        ? error.message
        : String(error);
    const fallback: RejectionPayload = { code: 'internal', message: fallbackMessage };
    throw fallback;
  }
}

/**
 * EnvVarMeta 契约上不存在 `value` 字段 —— 明文只能经 reveal_env_var 获取。
 *
 * 运行时校验是安全边界的第二道闸（第一道在 Rust）：
 * 1. 显式拒绝携带 `value` 自有属性的返回值 —— 一旦 Rust 序列化回归或
 *    mock 配错把明文带进列表，在此拦截而不是流入 Zustand / React DevTools；
 * 2. 白名单复制 8 个契约字段 —— 上游多余字段不会进入前端状态。
 */
function parseEnvVarMeta(value: unknown, label: string): EnvVarMeta {
  if (!isRecord(value)) {
    throw new Error(`${label} 返回了无效的 EnvVarMeta 契约`);
  }
  if ('value' in value) {
    throw new Error(`${label} 携带了禁止的 value 字段（列表契约不得包含敏感明文）`);
  }
  if (typeof value.name !== 'string' || value.name.trim().length === 0) {
    throw new Error(`${label} 的 name 必须是非空字符串`);
  }
  if (typeof value.kind !== 'string' || !ENV_VALUE_KINDS.includes(value.kind as EnvValueKind)) {
    throw new Error(`${label} 的 kind 无效`);
  }
  if (typeof value.hive !== 'string' || !ENV_HIVES.includes(value.hive as EnvHive)) {
    throw new Error(`${label} 的 hive 无效`);
  }
  if (
    typeof value.canEdit !== 'boolean' ||
    typeof value.canDelete !== 'boolean' ||
    typeof value.sensitive !== 'boolean'
  ) {
    throw new Error(`${label} 的 canEdit/canDelete/sensitive 必须是布尔值`);
  }
  if (value.preview !== null && typeof value.preview !== 'string') {
    throw new Error(`${label} 的 preview 必须是 string 或 null`);
  }
  if (typeof value.revision !== 'string' || value.revision.length === 0) {
    throw new Error(`${label} 的 revision 必须是非空字符串`);
  }
  // 白名单构造：显式枚举，绝不透传上游对象的其余字段
  return {
    name: value.name,
    kind: value.kind as EnvValueKind,
    hive: value.hive as EnvHive,
    canEdit: value.canEdit,
    canDelete: value.canDelete,
    sensitive: value.sensitive,
    preview: value.preview,
    revision: value.revision,
  };
}

/**
 * RevealedValue 契约校验：reveal_env_var 必须同时返回完整明文与读取时的
 * revision（F-01 编辑陈旧判定依赖后者）。白名单构造，绝不透传上游多余字段。
 */
function parseRevealedValue(value: unknown, label: string): RevealedValue {
  if (
    !isRecord(value) ||
    typeof value.value !== 'string' ||
    typeof value.revision !== 'string' ||
    value.revision.length === 0
  ) {
    throw new Error(`${label} 返回了无效的 RevealedValue 契约`);
  }
  return { value: value.value, revision: value.revision };
}

function parseEnvVarMetas(values: unknown, label: string): EnvVarMeta[] {
  if (!Array.isArray(values)) {
    throw new Error(`${label} 返回了无效的 EnvVarMeta[] 契约`);
  }
  return values.map((item) => parseEnvVarMeta(item, label));
}

function parseEnvVarSnapshot(value: unknown): EnvVarSnapshot {
  if (!isRecord(value)) {
    throw new Error('list_all_env_vars 返回了无效的 EnvVarSnapshot 契约');
  }
  // 旧版后端可能不返回 capturedAt；缺失或类型不符时回退 0，保持向后兼容。
  const capturedAt = typeof value.capturedAt === 'number' ? value.capturedAt : 0;
  return {
    system: parseEnvVarMetas(value.system, 'list_all_env_vars.system'),
    user: parseEnvVarMetas(value.user, 'list_all_env_vars.user'),
    capturedAt,
  };
}

/**
 * 解析 Rust `BackupOutcome`（外部标签枚举）：`{created}` / `"skipped"` / `{failed}`。
 *
 * 只认 `created` 的有效形状（字符串路径）：`{created: null}` 这类「键对值错」
 * 会让 `backupFailed` 静默判成非失败，等于把上游回归吞掉。
 */
function parseBackupOutcome(value: unknown, label: string): BackupOutcome {
  if (value === 'skipped') return 'skipped';
  if (isRecord(value)) {
    if (typeof value.created === 'string') return { created: value.created };
    if (typeof value.failed === 'string') return { failed: value.failed };
  }
  // 文案统一挂在 WriteOutcome.backup 上：调用方只认一个入口，报错定位到具体字段。
  throw new Error(`${label} 返回了无效的 WriteOutcome.backup 契约`);
}

/**
 * 写方法返回值的入口校验。
 *
 * `undefined` 是**兼容回退**：旧后端（或在 E2E mock 未接线时）返回空值，
 * 规范化为 `"skipped"`（「本次无需备份」）而不是报错 —— 把缺字段当成
 * 备份失败会在状态栏误报「保存成功（备份失败）」，比不显示更糟。
 * 其余非法形状（有 `backup` 但形状不对、或连 `backup` 都没有）一律拒绝，
 * 绝不透传。
 *
 * 本层文案**刻意与 [`parseBackupOutcome`] 不同**（本层提「缺少 backup 字段」，
 * 下一层提 `.backup`）：两者是两个不同缺陷，调用方与测试都要能分辨。
 * 若两层文案互相包含，删掉本层守卫后下一层抛出的错误会「撞上」同一个断言，
 * 该分支就变成不可证的死代码 —— 测试同样按分支分别断言。
 */
function parseWriteOutcome(value: unknown, label: string): WriteOutcome {
  if (value === undefined) return { backup: 'skipped' };
  if (!isRecord(value) || !('backup' in value)) {
    throw new Error(`${label} 返回了无效的 WriteOutcome 契约（缺少 backup 字段）`);
  }
  return { backup: parseBackupOutcome(value.backup, label) };
}

const RESTORE_CHANGE_KINDS: readonly string[] = ['added', 'modified', 'removed', 'conflict'];

/** 单条恢复差异的运行时校验（hive / kind 白名单，name 非空）。 */
function parseRestoreChange(value: unknown, label: string): RestoreChange {
  if (
    !isRecord(value) ||
    (value.hive !== 'system' && value.hive !== 'user') ||
    typeof value.kind !== 'string' ||
    !RESTORE_CHANGE_KINDS.includes(value.kind) ||
    typeof value.name !== 'string' ||
    value.name.length === 0
  ) {
    throw new Error(`${label} 返回了无效的 RestoreChange 契约`);
  }
  return {
    hive: value.hive,
    name: value.name,
    kind: value.kind as RestoreChangeKind,
  };
}

/** 四个计数必须是非负整数，且与 changes 长度自洽（Rust 侧契约恒等）。 */
function parseCount(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new Error(`${label} 返回了无效的计数`);
  }
  return value;
}

function parseRestorePreview(value: unknown): RestorePreview {
  const label = 'preview_env_backup';
  if (!isRecord(value) || !Array.isArray(value.changes)) {
    throw new Error(`${label} 返回了无效的 RestorePreview 契约`);
  }
  return {
    changes: value.changes.map((item) => parseRestoreChange(item, label)),
    added: parseCount(value.added, label),
    modified: parseCount(value.modified, label),
    removed: parseCount(value.removed, label),
    conflicts: parseCount(value.conflicts, label),
  };
}

/** 备份列表项的运行时校验；`variableCount` 缺失时回退 0（该字段恒为 0，见契约）。 */
function parseEnvBackupInfo(value: unknown, label: string): EnvBackupInfo {
  if (
    !isRecord(value) ||
    typeof value.file !== 'string' ||
    typeof value.path !== 'string' ||
    typeof value.timestamp !== 'string' ||
    typeof value.sizeBytes !== 'number'
  ) {
    throw new Error(`${label} 返回了无效的 EnvBackupInfo 契约`);
  }
  return {
    file: value.file,
    path: value.path,
    timestamp: value.timestamp,
    sizeBytes: value.sizeBytes,
    variableCount: typeof value.variableCount === 'number' ? value.variableCount : 0,
  };
}

/** 恢复结果：`applied` / `skipped` 非负整数，`failures` 全为字符串。 */
function parseRestoreOutcome(value: unknown): RestoreOutcome {
  const label = 'restore_env_backup';
  if (!isRecord(value) || !Array.isArray(value.failures)) {
    throw new Error(`${label} 返回了无效的 RestoreOutcome 契约`);
  }
  if (!value.failures.every((item: unknown) => typeof item === 'string')) {
    throw new Error(`${label} 的 failures 必须是字符串数组`);
  }
  return {
    applied: parseCount(value.applied, label),
    skipped: parseCount(value.skipped, label),
    failures: value.failures as string[],
  };
}

/** 备份路径返回值：非空字符串（`backup_env_vars` 的成功路径）。 */
function parseBackupPath(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} 返回了无效的备份路径契约`);
  }
  return value;
}

// ── v6 历史契约 ─────────────────────────────────────────────

/** 事件动作（对应 core 的 `EventAction`，serde camelCase）。 */
export type EventAction =
  | 'save'
  | 'apply'
  | 'restore'
  | 'import'
  | 'envSet'
  | 'envAdd'
  | 'envRemove'
  | 'undo'
  | 'redo';

/** 事件结果（对应 core 的 `EventResult`）。 */
export type EventResult = 'success' | 'partial' | 'failure';

/** 单条事件（对应 core 的 `EventView`）。 */
export interface EventView {
  uid: string;
  createdAt: number;
  source: string;
  action: EventAction;
  hive: string | null;
  target: string | null;
  result: EventResult;
  /** 本次操作产生的快照 uid；`null` 表示没有关联快照（无法查看详情）。 */
  snapshotUid: string | null;
}

/** 一页事件（`total` 是满足条件的总行数，不是本页条数）。 */
export interface EventPage {
  items: EventView[];
  total: number;
}

/** 事件列表筛选（P1-1：补齐 spec §6.1 要求的全部维度）。 */
export interface EventFilter {
  limit?: number;
  offset?: number;
  action?: string;
  source?: string;
  /** `system` / `user` */
  hive?: string;
  /** 操作目标（变量名等） */
  target?: string;
  /** `success` / `partial` / `failure` */
  result?: string;
  /** 时间范围起点（含，Unix 毫秒） */
  fromMs?: number;
  /** 时间范围终点（含，Unix 毫秒） */
  toMs?: number;
}

/** 快照中的一行环境变量（对应 core 的 `EnvValueRow`）。 */
export interface HistoryEnvValue {
  hive: string;
  name: string;
  kind: string;
  /** 敏感值在 reveal=false 时为 `null`。 */
  value: string | null;
  revision: string;
  sensitive: boolean;
  restorable: boolean;
}

/** 快照视图（对应 core 的 `SnapshotView`）。 */
export interface SnapshotView {
  uid: string;
  createdAt: number;
  kind: string;
  system: PathEntry[];
  user: PathEntry[];
  env: HistoryEnvValue[];
}

/** 配置方案视图（对应 core 的 `ProfileView`）。 */
export interface ProfileView {
  uid: string;
  name: string;
  description: string | null;
  tags: string[];
  createdAt: number;
  updatedAt: number;
  lastAppliedAt: number | null;
  applyCount: number;
  version: number;
  snapshotUid: string | null;
}

/** 备份索引视图（对应 core 的 `BackupView`）。 */
export interface BackupView {
  uid: string;
  kind: string;
  createdAt: number;
  contentHash: string;
  sizeBytes: number;
  filePath: string;
  /** `.pebak`（DPAPI 加密）为 true，遗留 `.json` 为 false。 */
  encrypted: boolean;
  variableCount: number;
}

const EVENT_ACTIONS: readonly EventAction[] = [
  'save',
  'apply',
  'restore',
  'import',
  'envSet',
  'envAdd',
  'envRemove',
  'undo',
  'redo',
];
const EVENT_RESULTS: readonly EventResult[] = ['success', 'partial', 'failure'];

/** 校验 `EventPage` 契约；形状不合直接抛错，不做部分信任。 */
export function parseEventPage(value: unknown): EventPage {
  if (!isRecord(value) || !Array.isArray(value.items) || typeof value.total !== 'number') {
    throw new Error('history_list_events 返回了无效的分页契约');
  }
  const items = value.items.map((raw) => {
    if (!isRecord(raw)) throw new Error('事件项形状无效');
    const { uid, createdAt, source, hive, target } = raw;
    const action = raw.action;
    const result = raw.result;
    if (typeof uid !== 'string' || typeof createdAt !== 'number') {
      throw new Error('事件项 uid/createdAt 形状无效');
    }
    if (typeof source !== 'string') throw new Error('事件项 source 形状无效');
    if (typeof action !== 'string' || !EVENT_ACTIONS.includes(action as EventAction)) {
      throw new Error('事件项 action 非法');
    }
    if (typeof result !== 'string' || !EVENT_RESULTS.includes(result as EventResult)) {
      throw new Error('事件项 result 非法');
    }
    return {
      uid,
      createdAt,
      source,
      action: action as EventAction,
      hive: typeof hive === 'string' ? hive : null,
      target: typeof target === 'string' ? target : null,
      result: result as EventResult,
      snapshotUid: typeof raw.snapshotUid === 'string' ? raw.snapshotUid : null,
    };
  });
  return { items, total: value.total };
}

/** 校验 `SnapshotView`；**不信任上游的 value 字段**，照契约原样收下后由 UI 决定掩码。 */
export function parseSnapshotView(value: unknown, label = 'history'): SnapshotView {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的快照契约`);
  const { uid, createdAt, kind } = value;
  if (typeof uid !== 'string' || typeof createdAt !== 'number' || typeof kind !== 'string') {
    throw new Error(`${label} 的快照 uid/createdAt/kind 形状无效`);
  }
  if (!Array.isArray(value.env)) throw new Error(`${label} 的 env 必须是数组`);
  const env = value.env.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.hive !== 'string' ||
      typeof raw.name !== 'string' ||
      typeof raw.kind !== 'string' ||
      typeof raw.revision !== 'string' ||
      typeof raw.sensitive !== 'boolean' ||
      typeof raw.restorable !== 'boolean'
    ) {
      throw new Error(`${label} 的 env 条目形状无效`);
    }
    const v = raw.value;
    if (v !== null && typeof v !== 'string') {
      throw new Error(`${label} 的 env 值必须是字符串或 null`);
    }
    return {
      hive: raw.hive,
      name: raw.name,
      kind: raw.kind,
      value: v,
      revision: raw.revision,
      sensitive: raw.sensitive,
      restorable: raw.restorable,
    };
  });
  return {
    uid,
    createdAt,
    kind,
    system: parsePathEntries(value.system, `${label}.system`),
    user: parsePathEntries(value.user, `${label}.user`),
    env,
  };
}

/** 校验 `ProfileView[]`。 */
function parseProfileList(value: unknown, label: string): ProfileView[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须是数组`);
  return value.map((raw) => {
    if (!isRecord(raw)) throw new Error(`${label} 的条目形状无效`);
    const { uid, name, description, tags, createdAt, updatedAt, lastAppliedAt } = raw;
    if (
      typeof uid !== 'string' ||
      typeof name !== 'string' ||
      typeof createdAt !== 'number' ||
      typeof updatedAt !== 'number' ||
      typeof raw.applyCount !== 'number' ||
      typeof raw.version !== 'number'
    ) {
      throw new Error(`${label} 的条目字段形状无效`);
    }
    if (!Array.isArray(tags) || !tags.every((t) => typeof t === 'string')) {
      throw new Error(`${label} 的 tags 必须是字符串数组`);
    }
    if (description !== null && typeof description !== 'string') {
      throw new Error(`${label} 的 description 必须是字符串或 null`);
    }
    if (lastAppliedAt !== null && typeof lastAppliedAt !== 'number') {
      throw new Error(`${label} 的 lastAppliedAt 必须是数字或 null`);
    }
    const snapshotUid = raw.snapshotUid;
    if (snapshotUid !== null && typeof snapshotUid !== 'string') {
      throw new Error(`${label} 的 snapshotUid 必须是字符串或 null`);
    }
    return {
      uid,
      name,
      description,
      tags: tags as string[],
      createdAt,
      updatedAt,
      lastAppliedAt,
      applyCount: raw.applyCount,
      version: raw.version,
      snapshotUid,
    };
  });
}

/** 校验 `BackupView[]`。 */
function parseBackupList(value: unknown, label: string): BackupView[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须是数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.uid !== 'string' ||
      typeof raw.kind !== 'string' ||
      typeof raw.createdAt !== 'number' ||
      typeof raw.contentHash !== 'string' ||
      typeof raw.sizeBytes !== 'number' ||
      typeof raw.filePath !== 'string' ||
      typeof raw.encrypted !== 'boolean' ||
      typeof raw.variableCount !== 'number'
    ) {
      throw new Error(`${label} 的条目形状无效`);
    }
    return {
      uid: raw.uid,
      kind: raw.kind,
      createdAt: raw.createdAt,
      contentHash: raw.contentHash,
      sizeBytes: raw.sizeBytes,
      filePath: raw.filePath,
      encrypted: raw.encrypted,
      variableCount: raw.variableCount,
    };
  });
}

/* ────────────────────────── M2：检索 / 统计 / 对比 / 引用图 ────────────────────────── */

/** 一条搜索命中（对应 core 的 `db::search::SearchHit`）。 */
export interface SearchHit {
  /** 来源类型：`path` / `envName` / `profile` */
  kind: string;
  /** 指向对象（快照 uid 带 `#hive` 后缀，或配置 uid） */
  refUid: string;
  /** 高亮片段（FTS5 `snippet()`，命中处用 `[]` 包裹） */
  snippet: string;
  /** 排名，越小越相关 */
  rank: number;
}

/** 单日统计点。 */
export interface DailyPoint {
  /** `YYYY-MM-DD` */
  day: string;
  pathItems: number;
  envItems: number;
  events: number;
  conflicts: number;
}

/** 最常改变的变量。 */
export interface TargetCount {
  target: string;
  count: number;
}

/** 配置应用统计。 */
export interface ProfileApply {
  name: string;
  applyCount: number;
  lastAppliedAt: number | null;
}

/**
 * 统计面板数据。
 *
 * **口径**：`days` 是**历史**序列（某天的 `pathItems` 是「那天最后一次保存时的
 * 条目数」）；`currentPathItems` / `currentAvgLen` 是**现在**的状态。
 * UI 必须分开显示，否则用户会把「今天最后一次保存」误读成「当前」。
 */
export interface Dashboard {
  days: DailyPoint[];
  topTargets: TargetCount[];
  profileApplies: ProfileApply[];
  currentAvgLen: number;
  currentPathItems: number;
}

/** 顺序变化的一条。 */
export interface ReorderEntry {
  path: string;
  leftIndex: number;
  rightIndex: number;
}

/** 启用状态变化的一条。 */
export interface ToggleEntry {
  path: string;
  leftEnabled: boolean;
  rightEnabled: boolean;
}

/** 单侧（system 或 user）的 PATH 差异。 */
export interface PathDiff {
  added: PathEntry[];
  removed: PathEntry[];
  reordered: ReorderEntry[];
  toggled: ToggleEntry[];
}

/**
 * 对比结果（N-8：system 与 user **分开**）。
 *
 * `env_*` 三字段本轮**恒为空数组**（T-3）——UI 必须显示
 * 「环境变量对比将在 M3 提供」，而不是让用户以为「env 没变化」。
 */
export interface CompareResult {
  system: PathDiff;
  user: PathDiff;
  envAdded: string[];
  envRemoved: string[];
  envModified: string[];
}

/** 一条引用边。 */
export interface RefEdge {
  fromHive: string;
  fromName: string;
  toName: string;
  depth: number;
}

/** 一次扫描记录。 */
export interface ScanRunView {
  uid: string;
  createdAt: number;
  conflicts: string[];
  toolCount: number;
}

/** 「首次出现」。 */
export interface FirstSeen {
  conflict: string;
  runUid: string;
  seenAt: number;
}

/** 「最近出现」。 */
export interface LastSeen {
  conflict: string;
  runUid: string;
  seenAt: number;
}

/** 两次扫描的差异。 */
export interface ScanRunDiff {
  appeared: string[];
  disappeared: string[];
  persistent: string[];
  lastSeen: LastSeen[];
  firstSeen: FirstSeen[];
}

const isStringArray = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((x) => typeof x === 'string');

/** 校验 `SearchHit[]`。 */
export function parseSearchHits(value: unknown, label = 'search'): SearchHit[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.kind !== 'string' ||
      typeof raw.refUid !== 'string' ||
      typeof raw.snippet !== 'string' ||
      typeof raw.rank !== 'number'
    ) {
      throw new Error(`${label} 的命中项形状无效`);
    }
    return {
      kind: raw.kind,
      refUid: raw.refUid,
      snippet: raw.snippet,
      rank: raw.rank,
    };
  });
}

/** 校验 `Dashboard`。 */
export function parseDashboard(value: unknown, label = 'stats_dashboard'): Dashboard {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的统计契约`);
  if (!Array.isArray(value.days)) throw new Error(`${label} 的 days 必须是数组`);
  const days = value.days.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.day !== 'string' ||
      typeof raw.pathItems !== 'number' ||
      typeof raw.envItems !== 'number' ||
      typeof raw.events !== 'number' ||
      typeof raw.conflicts !== 'number'
    ) {
      throw new Error(`${label} 的 days 条目形状无效`);
    }
    return {
      day: raw.day,
      pathItems: raw.pathItems,
      envItems: raw.envItems,
      events: raw.events,
      conflicts: raw.conflicts,
    };
  });
  if (!Array.isArray(value.topTargets)) throw new Error(`${label} 的 topTargets 必须是数组`);
  const topTargets = value.topTargets.map((raw) => {
    if (!isRecord(raw) || typeof raw.target !== 'string' || typeof raw.count !== 'number') {
      throw new Error(`${label} 的 topTargets 条目形状无效`);
    }
    return { target: raw.target, count: raw.count };
  });
  if (!Array.isArray(value.profileApplies)) {
    throw new Error(`${label} 的 profileApplies 必须是数组`);
  }
  const profileApplies = value.profileApplies.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.name !== 'string' ||
      typeof raw.applyCount !== 'number' ||
      (raw.lastAppliedAt !== null && typeof raw.lastAppliedAt !== 'number')
    ) {
      throw new Error(`${label} 的 profileApplies 条目形状无效`);
    }
    return {
      name: raw.name,
      applyCount: raw.applyCount,
      lastAppliedAt: raw.lastAppliedAt,
    };
  });
  if (typeof value.currentAvgLen !== 'number' || typeof value.currentPathItems !== 'number') {
    throw new Error(`${label} 的 current* 字段必须是数字`);
  }
  return {
    days,
    topTargets,
    profileApplies,
    currentAvgLen: value.currentAvgLen,
    currentPathItems: value.currentPathItems,
  };
}

/** 校验单侧 `PathDiff`。 */
function parsePathDiff(value: unknown, label: string): PathDiff {
  if (!isRecord(value)) throw new Error(`${label} 必须是对象`);
  if (!Array.isArray(value.reordered)) throw new Error(`${label}.reordered 必须是数组`);
  if (!Array.isArray(value.toggled)) throw new Error(`${label}.toggled 必须是数组`);
  const reordered = value.reordered.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.path !== 'string' ||
      typeof raw.leftIndex !== 'number' ||
      typeof raw.rightIndex !== 'number'
    ) {
      throw new Error(`${label}.reordered 条目形状无效`);
    }
    return { path: raw.path, leftIndex: raw.leftIndex, rightIndex: raw.rightIndex };
  });
  const toggled = value.toggled.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.path !== 'string' ||
      typeof raw.leftEnabled !== 'boolean' ||
      typeof raw.rightEnabled !== 'boolean'
    ) {
      throw new Error(`${label}.toggled 条目形状无效`);
    }
    return {
      path: raw.path,
      leftEnabled: raw.leftEnabled,
      rightEnabled: raw.rightEnabled,
    };
  });
  return {
    added: parsePathEntries(value.added, `${label}.added`),
    removed: parsePathEntries(value.removed, `${label}.removed`),
    reordered,
    toggled,
  };
}

/** 校验 `CompareResult`（N-8：双 hive 都必须存在）。 */
export function parseCompareResult(value: unknown, label = 'compare_snapshots'): CompareResult {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的对比契约`);
  // 缺 system 或 user 一律报错：只比一侧是本轮修掉的 N-8 缺陷，
  // 不能让后端退回旧形状而前端静默接受
  if (value.system === undefined || value.user === undefined) {
    throw new Error(`${label} 必须同时返回 system 与 user 两侧`);
  }
  // 逐个校验并**收窄类型**：TS 不会把循环内守卫的结果带到 return，
  // 因此这里先取回局部变量、逐个断言，再放进返回值。
  const envAdded = value.envAdded;
  const envRemoved = value.envRemoved;
  const envModified = value.envModified;
  if (!isStringArray(envAdded)) throw new Error(`${label}.envAdded 必须是字符串数组`);
  if (!isStringArray(envRemoved)) throw new Error(`${label}.envRemoved 必须是字符串数组`);
  if (!isStringArray(envModified)) throw new Error(`${label}.envModified 必须是字符串数组`);
  return {
    system: parsePathDiff(value.system, `${label}.system`),
    user: parsePathDiff(value.user, `${label}.user`),
    envAdded,
    envRemoved,
    envModified,
  };
}

/** 校验 `RefEdge[]`。 */
export function parseRefEdges(value: unknown, label = 'refs'): RefEdge[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.fromHive !== 'string' ||
      typeof raw.fromName !== 'string' ||
      typeof raw.toName !== 'string' ||
      typeof raw.depth !== 'number'
    ) {
      throw new Error(`${label} 的引用边形状无效`);
    }
    return {
      fromHive: raw.fromHive,
      fromName: raw.fromName,
      toName: raw.toName,
      depth: raw.depth,
    };
  });
}

/** 校验 `string[][]`（引用环）。 */
export function parseCycleList(value: unknown, label = 'refs_cycles'): string[][] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (!isStringArray(raw)) throw new Error(`${label} 的每个环必须是字符串数组`);
    return raw;
  });
}

/** 校验 `ScanRunView[]`。 */
export function parseScanRuns(value: unknown, label = 'scan_history_list'): ScanRunView[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.uid !== 'string' ||
      typeof raw.createdAt !== 'number' ||
      typeof raw.toolCount !== 'number' ||
      !isStringArray(raw.conflicts)
    ) {
      throw new Error(`${label} 的扫描记录形状无效`);
    }
    return {
      uid: raw.uid,
      createdAt: raw.createdAt,
      conflicts: raw.conflicts,
      toolCount: raw.toolCount,
    };
  });
}

/** 校验 `ScanRunDiff`。 */
export function parseScanRunDiff(value: unknown, label = 'scan_history_diff'): ScanRunDiff {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的 diff 契约`);
  const appeared = value.appeared;
  const disappeared = value.disappeared;
  const persistent = value.persistent;
  if (!isStringArray(appeared)) throw new Error(`${label}.appeared 必须是字符串数组`);
  if (!isStringArray(disappeared)) throw new Error(`${label}.disappeared 必须是字符串数组`);
  if (!isStringArray(persistent)) throw new Error(`${label}.persistent 必须是字符串数组`);
  if (!Array.isArray(value.lastSeen)) throw new Error(`${label}.lastSeen 必须是数组`);
  if (!Array.isArray(value.firstSeen)) throw new Error(`${label}.firstSeen 必须是数组`);
  const parseSeen = (raw: unknown, kind: string) => {
    if (
      !isRecord(raw) ||
      typeof raw.conflict !== 'string' ||
      typeof raw.runUid !== 'string' ||
      typeof raw.seenAt !== 'number'
    ) {
      throw new Error(`${label}.${kind} 条目形状无效`);
    }
    return { conflict: raw.conflict, runUid: raw.runUid, seenAt: raw.seenAt };
  };
  return {
    appeared,
    disappeared,
    persistent,
    lastSeen: value.lastSeen.map((r: unknown) => parseSeen(r, 'lastSeen')),
    firstSeen: value.firstSeen.map((r: unknown) => parseSeen(r, 'firstSeen')),
  };
}

/* ────────────────────────── M3：同步中心 ────────────────────────── */

/** 导出结果（`core::db::sync::ExportReport`）。 */
export interface SyncExportReport {
  /** 本次导出的变更条数 */
  changeCount: number;
  /** 写出的文件路径 */
  path: string;
}

/** 导入结果（`core::db::sync::ImportReport`）。 */
export interface SyncImportReport {
  /** 实际应用的变更数（真正落库的条数） */
  applied: number;
  /** 因已存在而跳过的变更数（幂等） */
  skipped: number;
  /** 记录的冲突数 */
  conflicts: number;
  /**
   * 本版本还不支持落库、因而**未应用也未记录**的变更数。
   *
   * 界面必须显示这个数字：否则用户会把「全部同步成功」当成事实。
   */
  skippedUnsupported: number;
  /**
   * 因与本地配置同名而**自动改名**的条数（M4）。
   *
   * 改名只影响本地行；不显示的话，用户会发现配置名字变了却找不到原因。
   */
  renamedProfiles: number;
}

/** 同步中心首屏状态。 */
export interface SyncStatus {
  /** 待导出变更数 */
  pendingCount: number;
  /** 未解决冲突数 */
  unresolvedConflicts: number;
}

/** 冲突的展示形状（`core::db::sync::conflict::SyncConflictView`）。 */
export interface SyncConflict {
  id: number;
  entityType: string;
  entityUid: string;
  field: string | null;
  localValue: string | null;
  remoteValue: string | null;
  remoteDevice: string;
  createdAt: number;
  resolvedAt: number | null;
  resolution: string | null;
}

/** 校验 `SyncExportReport`。 */
export function parseSyncExportReport(value: unknown, label = 'sync_export'): SyncExportReport {
  if (!isRecord(value) || typeof value.changeCount !== 'number' || typeof value.path !== 'string') {
    throw new Error(`${label} 返回了无效的导出结果`);
  }
  return { changeCount: value.changeCount, path: value.path };
}

/** 校验 `SyncImportReport`。 */
export function parseSyncImportReport(value: unknown, label = 'sync_import'): SyncImportReport {
  if (
    !isRecord(value) ||
    typeof value.applied !== 'number' ||
    typeof value.skipped !== 'number' ||
    typeof value.conflicts !== 'number' ||
    typeof value.skippedUnsupported !== 'number' ||
    typeof value.renamedProfiles !== 'number'
  ) {
    throw new Error(`${label} 返回了无效的导入结果`);
  }
  return {
    applied: value.applied,
    skipped: value.skipped,
    conflicts: value.conflicts,
    skippedUnsupported: value.skippedUnsupported,
    renamedProfiles: value.renamedProfiles,
  };
}

/** 校验 `SyncStatus`。 */
export function parseSyncStatus(value: unknown, label = 'sync_status'): SyncStatus {
  if (
    !isRecord(value) ||
    typeof value.pendingCount !== 'number' ||
    typeof value.unresolvedConflicts !== 'number'
  ) {
    throw new Error(`${label} 返回了无效的同步状态`);
  }
  return {
    pendingCount: value.pendingCount,
    unresolvedConflicts: value.unresolvedConflicts,
  };
}

/** 校验 `SyncConflict[]`。 */
export function parseSyncConflicts(value: unknown, label = 'sync_list_conflicts'): SyncConflict[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.id !== 'number' ||
      typeof raw.entityType !== 'string' ||
      typeof raw.entityUid !== 'string' ||
      (raw.field !== null && typeof raw.field !== 'string') ||
      (raw.localValue !== null && typeof raw.localValue !== 'string') ||
      (raw.remoteValue !== null && typeof raw.remoteValue !== 'string') ||
      typeof raw.remoteDevice !== 'string' ||
      typeof raw.createdAt !== 'number' ||
      (raw.resolvedAt !== null && typeof raw.resolvedAt !== 'number') ||
      (raw.resolution !== null && typeof raw.resolution !== 'string')
    ) {
      throw new Error(`${label} 的冲突条目形状无效`);
    }
    return {
      id: raw.id,
      entityType: raw.entityType,
      entityUid: raw.entityUid,
      field: raw.field,
      localValue: raw.localValue,
      remoteValue: raw.remoteValue,
      remoteDevice: raw.remoteDevice,
      createdAt: raw.createdAt,
      resolvedAt: raw.resolvedAt,
      resolution: raw.resolution,
    };
  });
}

/** 唯一的 Tauri IPC 边界；组件和 Store 不再直接拼命令字符串。 */
export const backend = {
  loadSystemPaths: async () =>
    parseStringArray(await invoke<unknown>('load_system_paths'), 'load_system_paths'),
  loadUserPaths: async () =>
    parseStringArray(await invoke<unknown>('load_user_paths'), 'load_user_paths'),
  loadPathSnapshot: async () => parsePathSnapshot(await invoke<unknown>('load_path_snapshot')),
  loadDisabledState: async () => {
    const result = await invoke<unknown>('load_disabled_state');
    if (!Array.isArray(result) || result.length !== 2) {
      throw new Error('load_disabled_state 返回了无效的元组契约');
    }
    return [
      parseStringArray(result[0], 'load_disabled_state.system'),
      parseStringArray(result[1], 'load_disabled_state.user'),
    ] as [string[], string[]];
  },
  saveDisabledState: (system: string[], user: string[]) =>
    invoke<void>('save_disabled_state', { system, user }),
  savePathSnapshot: (system: PathEntry[] | null, user: PathEntry[] | null) =>
    invoke<void>('save_path_snapshot', { system, user }),
  /**
   * 唯一的「保存 PATH + sidecar」编排入口（Wave 3.0-B，F-08 收口）。
   *
   * core 侧负责：写前备份、乐观并发校验（`expected`）、顺序写（system→user）、
   * 广播、sidecar/pending 事务。GUI 只传数据并映射结果。
   *
   * @returns 经 `parseApplyOutcome` 运行时校验的 `ApplyOutcome`。
   */
  savePathWithSidecar: async (request: {
    system: PathEntryWrite | null;
    user: PathEntryWrite | null;
  }) => parseApplyOutcome(await invoke<unknown>('save_path_with_sidecar', request)),
  checkAdmin: () => invoke<boolean>('check_admin'),
  getPathCapabilities: async () =>
    parsePathCapabilities(await invoke<unknown>('get_path_capabilities')),
  validatePath: (path: string) => invoke<boolean>('validate_path', { path }),
  expandEnvVars: (path: string) => invoke<string>('expand_env_vars', { path }),
  /** 异步确认对话框（替代阻塞式 window.confirm）：关窗确认等路径专用，绝不阻塞 JS 线程。 */
  confirmDialog: (message: string) => tauriConfirm(message),
  broadcastEnvChange: () => invoke<void>('broadcast_env_change'),
  backupRegistry: (customDir: string | null = null) =>
    invoke<string | null>('backup_registry', { customDir }),
  readTextFile: (path: string) => invoke<string>('read_text_file', { path }),
  importFile: (path: string) => invoke<[PathEntry[], PathEntry[]]>('import_file', { path }),
  exportPathEntries: (sys: PathEntry[], usr: PathEntry[], format: string) =>
    invoke<string>('export_path_entries', { sys, usr, format }),
  cleanPathEntries: (entries: PathEntry[]) =>
    invoke<[PathEntry[], PathEntry[]]>('clean_path_entries', { entries }),
  scanPaths: (paths: string[], query = '') => invoke<ScanResult>('scan_paths', { paths, query }),
  /**
   * 配置方案已迁到 DB（P0-1 方案 A）。
   *
   * **不再使用 v5 文件通路的 `list_profiles` / `save_profile` / …**：
   * 那组命令读写的仍是 `~/.patheditor/profiles/*.json`，与 DB 是两个真相源。
   * 迁移之后继续用它会让 DB 数据永久冻结（`migration_imports` 保证不重导）。
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
  profileMarkAppliedByName: async (name: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_mark_applied_by_name', { name }));
    if (typeof result !== 'boolean') {
      throw new Error('profile_mark_applied_by_name 必须返回布尔值');
    }
    return result;
  },
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

  // ── v6 历史（spec §3.2.1：细粒度命令，前端只做形状校验） ────────────

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
   * 把历史快照恢复回注册表（P0-3）。
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
   * 显式 reveal 一个历史快照里的环境变量值（Task 7 / M1 欠账 P2-1）。
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
  /**
   * 把配置导出成 v5 `profiles/*.json` 格式的字符串（M1 欠账 P2-2）。
   *
   * 返回值可直接用 `save` 对话框写成 `.json` 文件，供降级到 v5 时使用。
   * **不写用户文件**：落盘由前端负责，Rust 侧只产出内容。
   */
  profileExportV5: async (uid: string): Promise<string> => {
    const result = await parseCoreError(invoke<unknown>('profile_export_v5', { uid }));
    if (typeof result !== 'string') {
      throw new Error('profile_export_v5 必须返回字符串');
    }
    return result;
  },
  /** 软删除配置方案（按 uid）；返回是否真的删除了（已删除返回 false）。 */
  profileDelete: async (uid: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_delete', { uid }));
    if (typeof result !== 'boolean') throw new Error('profile_delete 必须返回布尔值');
    return result;
  },
  /** 记录配置被应用（应用次数 +1）。注册表写入由 `applyProfile` 另行完成。 */
  profileMarkApplied: async (uid: string) => {
    const result = await parseCoreError(invoke<unknown>('profile_mark_applied', { uid }));
    if (typeof result !== 'boolean') throw new Error('profile_mark_applied 必须返回布尔值');
    return result;
  },
  /** 列出备份索引（`.pebak` 的变量数来自文件头，无需解密）。 */
  backupList: async () =>
    parseBackupList(await parseCoreError(invoke<unknown>('backup_list')), 'backup_list'),

  /* ────────────────────── M2：检索 / 统计 / 对比 / 引用图 / 冲突历史 ────────────────────── */

  /**
   * FTS5 全文检索。
   *
   * **空查询由后端直接返回空数组**（见 `core::db::search::search`），
   * 前端不应依赖这一点来省请求——搜索框为空时 UI 也不该发起调用
   * （Task 10 的要求），两层各守一次。
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

  /** 全量重建 `daily_stats`（Task 12 启动接线与调试用）。 */
  statsRebuild: async () => {
    const n = await parseCoreError(invoke<unknown>('stats_rebuild'));
    if (typeof n !== 'number') throw new Error('stats_rebuild 必须返回数字');
    return n;
  },

  /** 比较两个快照（N-8：system 与 user 分开返回）。 */
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

  /** 全量重建引用图（Task 12 启动接线与调试用）。 */
  refsRebuild: async () => {
    const n = await parseCoreError(invoke<unknown>('refs_rebuild'));
    if (typeof n !== 'number') throw new Error('refs_rebuild 必须返回数字');
    return n;
  },

  /** 全量重建 FTS 索引（Task 12 启动接线与调试用）。 */
  searchRebuild: async () => {
    const n = await parseCoreError(invoke<unknown>('search_rebuild'));
    if (typeof n !== 'number') throw new Error('search_rebuild 必须返回数字');
    return n;
  },

  /** 列出最近的扫描记录。 */
  scanHistoryList: async (limit = 50) =>
    parseScanRuns(
      await parseCoreError(invoke<unknown>('scan_history_list', { limit })),
      'scan_history_list',
    ),

  /** 比较两次扫描。 */
  scanHistoryDiff: async (aUid: string, bUid: string) =>
    parseScanRunDiff(
      await parseCoreError(invoke<unknown>('scan_history_diff', { aUid, bUid })),
      'scan_history_diff',
    ),

  /**
   * 记录一次扫描，返回 uid。
   *
   * `conflictsJson` / `toolsJson` 由调用方序列化后传入——core 侧不解析
   * `ScanResult`，这样命令对 `scan_paths` 的形状变化免疫。
   */
  scanHistoryRecord: async (conflictsJson: string, toolsJson = '[]') => {
    const uid = await parseCoreError(
      invoke<unknown>('scan_history_record', { conflictsJson, toolsJson }),
    );
    if (typeof uid !== 'string') throw new Error('scan_history_record 必须返回字符串');
    return uid;
  },

  /* ────────────────────── M3：同步中心 ────────────────────── */

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
   * 组件侧用 `type="password"` + 局部 state 持有它（与 M2 的 reveal 同口径）。
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

// ── 配置方案（DB 版，P0-1 方案 A） ────────────────────────

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
