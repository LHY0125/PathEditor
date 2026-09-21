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

/**
 * list_env_backups 的 mock 列表。`variableCount` 恒为 0（Rust 不解析内容）。
 * 两条记录的时间戳故意不同，用于验证列表顺序原样透传（前端不重排）。
 */
const envBackupsFixture = [
  {
    file: 'env_backup_20260922_120000_000.json',
    path: 'C:\\backups\\env_backup_20260922_120000_000.json',
    timestamp: '20260922_120000_000',
    sizeBytes: 2048,
    variableCount: 0,
  },
  {
    file: 'env_backup_20260921_090000_000.json',
    path: 'C:\\backups\\env_backup_20260921_090000_000.json',
    timestamp: '20260921_090000_000',
    sizeBytes: 1024,
    variableCount: 0,
  },
];

/**
 * preview_env_backup 的 mock 差异。
 *
 * 三个计数故意两两不等（新增 1 / 修改 0 / 删除 2 / 冲突 3），这样界面上任何
 * 计数对调都会让断言失败。含两条 removed，用于验收「单独列出将被删除的变量名」。
 */
const restorePreviewFixture = {
  changes: [
    { hive: 'user', name: 'NEW_ONLY', kind: 'added' },
    { hive: 'user', name: 'OLD_VAR', kind: 'removed' },
    { hive: 'system', name: 'LEGACY_HOME', kind: 'removed' },
    { hive: 'user', name: 'JAVA_HOME', kind: 'conflict' },
    { hive: 'system', name: 'windir', kind: 'conflict' },
    { hive: 'user', name: 'MY_TOKEN', kind: 'conflict' },
  ],
  added: 1,
  modified: 0,
  removed: 2,
  conflicts: 3,
};

/**
 * reveal_env_var 的 mock 返回值（F-01 新契约）：按变量名索引
 * `{ value, revision }`，revision 与 allEnvVarsFixture 快照一致。
 */
const revealedFixture: Record<string, { value: string; revision: string }> = {
  JAVA_HOME: { value: 'C:\\Java', revision: 'usr-java' },
  MY_TOKEN: { value: 'plaintext-secret-value', revision: 'usr-token' },
  windir: { value: 'C:\\WINDOWS', revision: 'sys-windir' },
  ADMIN_ONLY: { value: 'x', revision: 'sys-ro' },
};

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
          case 'reveal_env_var':
            // 新契约（F-01）：返回明文 + 读取时 revision（按名称查 fixture 快照）
            return ${JSON.stringify(revealedFixture)}[args?.name] ?? null;
          // Tauri 插件对话框：插件侧 confirm() 实际调 plugin:dialog|message，
          // 并把返回值与 okLabel（默认 Ok）比较得出布尔结果。默认返回 Cancel
          // （与「取消」一致）；测试置 window.__confirmResponse = true 走通确认链路
          // （正文经 __capturedCalls 记录供断言：args.message / args.title / args.kind）。
          case 'plugin:dialog|message':
            return window.__confirmResponse ? 'Ok' : 'Cancel';
          // 环境变量备份通路（Task 8）。返回值形状必须与 Rust 序列化一致：
          // BackupOutcome 是外部标签枚举 → {"created":"路径"} / "skipped" / {"failed":"原因"}。
          case 'backup_env_vars': return 'C:\\\\backups\\\\env_backup_20260922_120000_000.json';
          case 'list_env_backups': return ${JSON.stringify(envBackupsFixture)};
          case 'preview_env_backup': {
            if (window.__backupConflictOverride) {
              return {
                changes: [
                  { hive: 'user', name: 'JAVA_HOME', kind: 'conflict' },
                  { hive: 'user', name: 'MY_TOKEN', kind: 'added' }
                ],
                added: 1,
                modified: 0,
                removed: 0,
                conflicts: 1
              };
            }
            return ${JSON.stringify(restorePreviewFixture)};
          }
          case 'restore_env_backup':
            // 冲突契约（与 update_env_var 同源）：对象带 code 字段，前端按 code 判定。
            if (window.__conflictOverride && !args?.force) {
              throw {
                code: 'conflict',
                operation: 'restore_env_backup',
                hive: null,
                name: null,
                retryable: true,
                message: '备份后有外部修改，请确认是否强制覆盖'
              };
            }
            if (window.__restoreForbidden) {
              throw {
                code: 'permissionDenied',
                operation: 'restore_env_backup',
                hive: 'system',
                name: null,
                retryable: false,
                message: '打开系统注册表键失败：拒绝访问'
              };
            }
            return { applied: 3, skipped: 0, failures: [] };
          case 'update_env_var':
            // 冲突契约（F-06）：与 Rust CoreError 序列化一致，对象形状携带
            // code 字段（前端按 code 判定，不再匹配 [E_CONFLICT] 文本前缀）。
            // message 保留前缀仅为与真实 core 文案对齐的展示文本。
            if (window.__conflictOverride) {
              throw {
                code: 'conflict',
                operation: 'update_env_var',
                hive: 'user',
                name: args?.name ?? null,
                retryable: true,
                message: '[E_CONFLICT] 变量已被其他进程修改，请重新加载',
              };
            }
            return { backup: { created: 'C:\\\\backups\\\\env_backup_x.json' } };
          case 'create_env_var': return { backup: 'skipped' };
          case 'delete_env_var': return { backup: 'skipped' };
          default: throw new Error('Unexpected invoke: ' + cmd);
        }
      }
    };
  `;
}
