import { invoke } from '@tauri-apps/api/core';
import type { PathEntry, PathSnapshot } from '@/core/path-entry';
import type { PathCapabilities } from '@/core/path-capabilities';

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
};
