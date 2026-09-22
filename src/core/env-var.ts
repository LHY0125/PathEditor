/**
 * 通用环境变量的纯展示逻辑。
 *
 * 安全判定（保留 / 保护 / 敏感 / 权限）全部在 Rust 侧计算并下发，
 * 本模块不重复实现 —— 避免双端各写一份判定规则而产生分歧。
 */

export type EnvValueKind = 'string' | 'expandString' | 'unsupported';
export type EnvHive = 'system' | 'user';
export type HiveFilter = 'system' | 'user' | 'all';

/**
 * 一次写操作前的备份结果（Rust `backup::BackupOutcome` 的 serde 形状）。
 *
 * 是**外部标签枚举**（externally tagged），JSON 形如：
 * - `Created(PathBuf)` → `{ "created": "C:\\…\\env_backup_….json" }`
 * - `Skipped`          → `"skipped"`
 * - `Failed(String)`   → `{ "failed": "原因" }`
 */
export type BackupOutcome = { created: string } | 'skipped' | { failed: string };

/** 一次环境变量写操作的结果（Rust `registry::WriteOutcome`）。 */
export interface WriteOutcome {
  backup: BackupOutcome;
}

/**
 * 备份是否失败；未知形状（未来新增变体）按「非失败」处理，不影响写入成功语义。
 *
 * 判定只认 `failed` 自有属性 —— `{created}` 与 `{failed}` 是同一层级的不同键，
 * 不能用「是对象 → 失败」这种粗判，否则备份成功也会被报成失败。
 */
export function backupFailed(backup: BackupOutcome): string | null {
  return typeof backup === 'object' && backup !== null && 'failed' in backup
    ? (backup as { failed: string }).failed
    : null;
}

/**
 * Rust `ErrorCode` 的镜像（serde camelCase 序列化）。
 *
 * F-06：错误判定只看 `code`，不匹配 `message` 文本 —— 措辞变化不会静默
 * 破坏冲突检测等分支。未知 code 由 backend 解析层兜底为 `internal`。
 */
export type ErrorCode =
  | 'conflict'
  | 'reservedName'
  | 'protected'
  | 'unsupportedType'
  | 'permissionDenied'
  | 'notFound'
  | 'nameExists'
  | 'invalidName'
  | 'invalidValue'
  | 'io'
  | 'parse'
  | 'internal';

/** Rust `CoreError` 的前端契约（Tauri rejection 序列化形状）。 */
export interface CoreError {
  code: ErrorCode;
  operation: string;
  hive: EnvHive | null;
  name: string | null;
  retryable: boolean;
  /** 面向用户的中文完整句；判定不得匹配此字段。 */
  message: string;
}

export interface EnvVarMeta {
  name: string;
  kind: EnvValueKind;
  hive: EnvHive;
  canEdit: boolean;
  canDelete: boolean;
  sensitive: boolean;
  preview: string | null;
  revision: string;
}

export interface EnvVarSnapshot {
  system: EnvVarMeta[];
  user: EnvVarMeta[];
  /** 快照采集时刻（Unix 毫秒）。两个 hive 是先后两次读取，不是原子快照；缺失时为 0。 */
  capturedAt: number;
}

/** 完整明文及其读取时的 revision（与 Rust `RevealedValue` 契约一致）。 */
export type RevealedValue = {
  value: string;
  revision: string;
};

const MASK_PLACEHOLDER = '••••••••';
const UNSUPPORTED_PLACEHOLDER = '(不支持的注册表类型)';

/** 唯一键：同名变量可能在两个 hive 同时存在。 */
export function envVarKey(meta: EnvVarMeta): string {
  return `${meta.hive}:${meta.name}`;
}

/**
 * 按键从快照派生最新 meta。
 *
 * 弹窗/壳层不得长期持有 EnvVarMeta 本体 —— 冲突刷新等场景下快照会换代，
 * 固化旧 meta 会拿旧 revision 反复提交（F-02）。提交前必须经此函数取最新。
 */
export function findMetaByKey(snapshot: EnvVarSnapshot, key: string): EnvVarMeta | null {
  return [...snapshot.system, ...snapshot.user].find((m) => envVarKey(m) === key) ?? null;
}

/**
 * 前端预校验变量名，与 Rust `validate_env_name` 规则保持一致。
 * 返回错误文案，或 null 表示通过。
 */
export function validateVarName(name: string): string | null {
  if (name.trim().length === 0) return '变量名不能为空';
  if (name.includes('\0')) return '变量名不能包含 null 字节';
  if (name.includes('=')) return '变量名不能包含等号';
  return null;
}

/** 计算某变量在表格中的展示值。 */
export function displayValue(
  meta: EnvVarMeta,
  revealedValue: string | null,
  unsupportedPlaceholder: string = UNSUPPORTED_PLACEHOLDER,
): string {
  if (meta.kind === 'unsupported') return unsupportedPlaceholder;
  if (meta.sensitive) {
    return revealedValue === null ? MASK_PLACEHOLDER : revealedValue;
  }
  return meta.preview ?? '';
}

/**
 * 按来源筛选与关键字过滤变量列表。
 *
 * 搜索**只匹配变量名，绝不匹配值** —— 匹配值等于把密钥拿去比较，
 * 且命中与否本身就会泄露信息。
 */
export function filterEnvVars(
  snapshot: EnvVarSnapshot,
  filter: HiveFilter,
  query: string,
): EnvVarMeta[] {
  const source: EnvVarMeta[] =
    filter === 'system'
      ? snapshot.system
      : filter === 'user'
        ? snapshot.user
        : [...snapshot.system, ...snapshot.user];

  const trimmed = query.trim().toLowerCase();
  // 空查询也要返回副本：`system` / `user` 分支的 source 是快照本体引用，
  // 调用方（store）若对结果做原地排序或增删，会直接污染共享快照。
  if (trimmed.length === 0) return [...source];

  return source.filter((meta) => meta.name.toLowerCase().includes(trimmed));
}
