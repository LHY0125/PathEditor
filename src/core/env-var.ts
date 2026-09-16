/**
 * 通用环境变量的纯展示逻辑。
 *
 * 安全判定（保留 / 保护 / 敏感 / 权限）全部在 Rust 侧计算并下发，
 * 本模块不重复实现 —— 避免双端各写一份判定规则而产生分歧。
 */

export type EnvValueKind = 'string' | 'expandString' | 'unsupported';
export type EnvHive = 'system' | 'user';
export type HiveFilter = 'system' | 'user' | 'all';

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
}

const MASK_PLACEHOLDER = '••••••••';
const UNSUPPORTED_PLACEHOLDER = '(不支持的注册表类型)';

/** 唯一键：同名变量可能在两个 hive 同时存在。 */
export function envVarKey(meta: EnvVarMeta): string {
  return `${meta.hive}:${meta.name}`;
}

/**
 * 敏感值的固定占位符。
 *
 * 刻意不接受真实值作参数 —— 确保明文不会经过本函数（也就不会进入
 * 调用栈、日志或调试器）。
 */
export function maskValue(): string {
  return MASK_PLACEHOLDER;
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

function hiveOf(meta: EnvVarMeta): string {
  return meta.hive === 'system' ? 'system' : 'user';
}

/** 计算某变量在表格中的展示值。 */
export function displayValue(meta: EnvVarMeta, revealedValue: string | null): string {
  if (meta.kind === 'unsupported') return UNSUPPORTED_PLACEHOLDER;
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

/** 筛选值是否属于已知范围（用于运行时校验）。 */
export function isHiveFilter(value: unknown): value is HiveFilter {
  return value === 'system' || value === 'user' || value === 'all';
}

export { hiveOf };
