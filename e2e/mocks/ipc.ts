export function createIpcMock() {
  return `
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        switch (cmd) {
          case 'check_admin': return true;
          case 'load_system_paths': return ['C:\\\\Windows', 'C:\\\\Program Files'];
          case 'load_user_paths': return ['C:\\\\Users\\\\me\\\\AppData'];
          case 'load_disabled_state': return [[], []];
          case 'save_system_paths': return undefined;
          case 'save_user_paths': return undefined;
          case 'save_disabled_state': return undefined;
          case 'backup_registry': return 'C:\\\\backup\\\\path.txt';
          case 'broadcast_env_change': return undefined;
          case 'validate_path': return true;
          case 'expand_env_vars': return 'C:\\\\Expanded';
          case 'read_text_file': return '';
          case 'get_appdata_dir': return 'C:\\\\appdata';
          case 'scan_conflicts': return [];
          case 'scan_tools': return [];
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
