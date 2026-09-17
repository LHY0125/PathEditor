import { invoke } from '@tauri-apps/api/core';
import type { PathEntry, PathSnapshot } from '@/core/path-entry';
import type { PathCapabilities } from '@/core/path-capabilities';
import type { EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot } from '@/core/env-var';

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
  return {
    system: parseEnvVarMetas(value.system, 'list_all_env_vars.system'),
    user: parseEnvVarMetas(value.user, 'list_all_env_vars.user'),
  };
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
  saveSystemPaths: (paths: string[], original: string[]) =>
    invoke<void>('save_system_paths', { paths, original }),
  saveUserPaths: (paths: string[], original: string[]) =>
    invoke<void>('save_user_paths', { paths, original }),
  saveDisabledState: (system: string[], user: string[]) =>
    invoke<void>('save_disabled_state', { system, user }),
  savePathSnapshot: (system: PathEntry[] | null, user: PathEntry[] | null) =>
    invoke<void>('save_path_snapshot', { system, user }),
  checkAdmin: () => invoke<boolean>('check_admin'),
  getPathCapabilities: async () =>
    parsePathCapabilities(await invoke<unknown>('get_path_capabilities')),
  validatePath: (path: string) => invoke<boolean>('validate_path', { path }),
  expandEnvVars: (path: string) => invoke<string>('expand_env_vars', { path }),
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
  listProfiles: () => invoke<ProfileMeta[]>('list_profiles'),
  saveProfile: (name: string, sys: PathEntry[], user: PathEntry[]) =>
    invoke<void>('save_profile', { name, sys, user }),
  loadProfile: (name: string) => invoke<ProfileData>('load_profile', { name }),
  deleteProfile: (name: string) => invoke<void>('delete_profile', { name }),
  renameProfile: (oldName: string, newName: string) =>
    invoke<void>('rename_profile', { oldName, newName }),
  listAllEnvVars: async () => parseEnvVarSnapshot(await invoke<unknown>('list_all_env_vars')),
  revealEnvVar: (hive: EnvHive, name: string) => invoke<string>('reveal_env_var', { hive, name }),
  updateEnvVar: (hive: EnvHive, name: string, value: string, expectedRevision: string) =>
    invoke<void>('update_env_var', { hive, name, value, expectedRevision }),
  createEnvVar: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) =>
    invoke<void>('create_env_var', { hive, name, value, kind }),
  deleteEnvVar: (hive: EnvHive, name: string, expectedRevision: string) =>
    invoke<void>('delete_env_var', { hive, name, expectedRevision }),
};
