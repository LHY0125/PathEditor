import { readFileSync } from 'node:fs';

const pathCapabilities = JSON.parse(
  readFileSync(new URL('../../tests/fixtures/path-capabilities.json', import.meta.url), 'utf8'),
) as {
  canReadSystem: boolean;
  canWriteSystem: boolean;
  canReadUser: boolean;
  canWriteUser: boolean;
};

export type IpcOverrides = Partial<Record<string, unknown>>;

export function createIpcMock(overrides: IpcOverrides = {}) {
  return `
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        const overrides = ${JSON.stringify(overrides)};
        if (cmd in overrides) return overrides[cmd];
        switch (cmd) {
          case 'check_admin': return true;
          case 'get_path_capabilities': return ${JSON.stringify({ ...pathCapabilities, canWriteSystem: true })};
          case 'load_system_paths': return ['C:\\\\Windows', 'C:\\\\Program Files'];
          case 'load_user_paths': return ['C:\\\\Users\\\\me\\\\AppData'];
          case 'load_path_snapshot': return {
            system: [
              { path: 'C:\\\\Windows', enabled: true },
              { path: 'C:\\\\Program Files', enabled: true }
            ],
            user: [{ path: 'C:\\\\Users\\\\me\\\\AppData', enabled: true }]
          };
          case 'load_disabled_state': return [[], []];
          case 'save_system_paths': return undefined;
          case 'save_user_paths': return undefined;
          case 'save_disabled_state': return undefined;
          case 'save_path_snapshot': return undefined;
          case 'backup_registry': return 'C:\\\\backup\\\\path.txt';
          case 'broadcast_env_change': return undefined;
          case 'validate_path': return true;
          case 'expand_env_vars': return 'C:\\\\Expanded';
          case 'read_text_file': return '';
          case 'import_file': return [[], []];
          case 'export_path_entries': return '';
          case 'clean_path_entries': return [args?.entries ?? [], []];
          case 'get_appdata_dir': return 'C:\\\\appdata';
          case 'scan_conflicts': return [];
          case 'scan_tools': return [];
          case 'scan_paths': return { conflicts: [], tools: [] };
          case 'list_profiles': return [];
          case 'save_profile': return undefined;
          case 'load_profile': return null;
          case 'delete_profile': return undefined;
          case 'rename_profile': return undefined;
          default: throw new Error('Unexpected invoke: ' + cmd);
        }
      }
    };
  `;
}
