import type { PathEntry, PathSnapshot } from '@/core/path-entry';
import type { PathCapabilities } from '@/core/path-capabilities';
import type {
  BackupOutcome,
  CoreError,
  EnvHive,
  EnvValueKind,
  EnvVarMeta,
  EnvVarSnapshot,
  ErrorCode,
  RevealedValue,
  WriteOutcome,
} from '@/core/env-var';
import type {
  EnvBackupInfo,
  RestoreChange,
  RestoreChangeKind,
  RestoreOutcome,
  RestorePreview,
} from '@/core/env-backup';

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isPathEntry(value: unknown): value is PathEntry {
  return isRecord(value) && typeof value.path === 'string' && typeof value.enabled === 'boolean';
}

export function parsePathEntries(value: unknown, label: string): PathEntry[] {
  if (!Array.isArray(value) || !value.every(isPathEntry)) {
    throw new Error(`${label} 返回了无效的 PathEntry[] 契约`);
  }
  return value.map((entry) => ({ path: entry.path, enabled: entry.enabled }));
}

export function parseStringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || !value.every((item) => typeof item === 'string')) {
    throw new Error(`${label} 返回了无效的字符串数组契约`);
  }
  return value;
}

export function parsePathSnapshot(value: unknown): PathSnapshot {
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
export function parseHiveOutcome(value: unknown, label: string): HiveOutcome {
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
export function parseSidecarOutcome(value: unknown): SidecarOutcome {
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
export function parseApplyOutcome(value: unknown): ApplyOutcome {
  if (!isRecord(value)) throw new Error('save_path_with_sidecar 返回了无效的跨层契约');
  return {
    system: parseHiveOutcome(value.system, 'save_path_with_sidecar.system'),
    user: parseHiveOutcome(value.user, 'save_path_with_sidecar.user'),
    sidecar: parseSidecarOutcome(value.sidecar),
  };
}

export function parsePathCapabilities(value: unknown): PathCapabilities {
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
 * 把 Tauri invocation 的 rejection 解析为结构化 `CoreError`。
 *
 * 双形状兼容：
 * - env 与 v6 DB/同步命令返回 `CoreError` 对象 → 白名单校验 `code` 后结构化透传；
 * - PATH 命令仍返回纯文本 `String` → 兜底为 `{code:'internal', message: 原文}`。
 *   PATH 命令迁移到 `CoreError` 后可收紧为单形状。
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
export function parseEnvVarMeta(value: unknown, label: string): EnvVarMeta {
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
 * revision（编辑陈旧判定依赖后者）。白名单构造，绝不透传上游多余字段。
 */
export function parseRevealedValue(value: unknown, label: string): RevealedValue {
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

export function parseEnvVarMetas(values: unknown, label: string): EnvVarMeta[] {
  if (!Array.isArray(values)) {
    throw new Error(`${label} 返回了无效的 EnvVarMeta[] 契约`);
  }
  return values.map((item) => parseEnvVarMeta(item, label));
}

export function parseEnvVarSnapshot(value: unknown): EnvVarSnapshot {
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
export function parseBackupOutcome(value: unknown, label: string): BackupOutcome {
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
export function parseWriteOutcome(value: unknown, label: string): WriteOutcome {
  if (value === undefined) return { backup: 'skipped' };
  if (!isRecord(value) || !('backup' in value)) {
    throw new Error(`${label} 返回了无效的 WriteOutcome 契约（缺少 backup 字段）`);
  }
  return { backup: parseBackupOutcome(value.backup, label) };
}

const RESTORE_CHANGE_KINDS: readonly string[] = ['added', 'modified', 'removed', 'conflict'];

/** 单条恢复差异的运行时校验（hive / kind 白名单，name 非空）。 */
export function parseRestoreChange(value: unknown, label: string): RestoreChange {
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
export function parseCount(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new Error(`${label} 返回了无效的计数`);
  }
  return value;
}

export function parseRestorePreview(value: unknown): RestorePreview {
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
export function parseEnvBackupInfo(value: unknown, label: string): EnvBackupInfo {
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
export function parseRestoreOutcome(value: unknown): RestoreOutcome {
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
export function parseBackupPath(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} 返回了无效的备份路径契约`);
  }
  return value;
}
