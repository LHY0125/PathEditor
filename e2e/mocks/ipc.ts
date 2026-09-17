import { readFileSync } from 'node:fs';

const pathCapabilities = JSON.parse(
  readFileSync(new URL('../../tests/fixtures/path-capabilities.json', import.meta.url), 'utf8'),
) as {
  canReadSystem: boolean;
  canWriteSystem: boolean;
  canReadUser: boolean;
  canWriteUser: boolean;
};

/**
 * list_all_env_vars 的 mock 快照。
 *
 * 契约：Path 已被 Rust 侧过滤，绝不会出现在返回值中 —— 因此这里故意不含 Path，
 * 前端若把 Path 泄漏进「全部变量」列表，E2E 用例会直接失败。
 * 用 const + JSON.stringify 注入，避免在模板字符串里手写 Windows 反斜杠转义。
 */
const allEnvVarsFixture = {
  system: [
    {
      name: 'windir',
      kind: 'string',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: 'C:\\WINDOWS',
      revision: 'sys-windir',
    },
    {
      name: 'SYS_BINARY',
      kind: 'unsupported',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: null,
      revision: 'sys-bin',
    },
    {
      name: 'ADMIN_ONLY',
      kind: 'string',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: 'x',
      revision: 'sys-ro',
    },
  ],
  user: [
    {
      name: 'JAVA_HOME',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: false,
      preview: 'C:\\Java',
      revision: 'usr-java',
    },
    {
      name: 'MY_TOKEN',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: true,
      preview: null,
      revision: 'usr-token',
    },
  ],
};

export type IpcOverrides = Partial<Record<string, unknown>>;

export function createIpcMock(overrides: IpcOverrides = {}) {
  return `
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        // E2E 调用捕获：供断言 expectedRevision 等参数使用
        window.__capturedCalls = window.__capturedCalls || [];
        window.__capturedCalls.push({ cmd, args });

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
          case 'list_all_env_vars': return ${JSON.stringify(allEnvVarsFixture)};
          case 'reveal_env_var': return 'plaintext-secret-value';
          case 'update_env_var':
            // 冲突契约：与 Rust 侧一致，携带 [E_CONFLICT] 前缀（前端按前缀匹配）
            if (window.__conflictOverride) {
              throw new Error('[E_CONFLICT] 变量已被其他进程修改，请重新加载');
            }
            return undefined;
          case 'create_env_var': return undefined;
          case 'delete_env_var': return undefined;
          default: throw new Error('Unexpected invoke: ' + cmd);
        }
      }
    };
  `;
}
