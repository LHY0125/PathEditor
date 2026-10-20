interface Window {
  __TAURI_INTERNALS__?: {
    invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  };
  /** 历史相关 mock 的注入点（P1-6 E2E）。每个用例按需设置。 */
  __historyPageFixture?: {
    items: Array<{
      uid: string;
      createdAt: number;
      source: string;
      action: string;
      hive: string | null;
      target: string | null;
      result: string;
      snapshotUid: string | null;
    }>;
    total: number;
  };
  /** `null` 表示没有可展开的快照。 */
  __snapshotFixture?: {
    uid: string;
    createdAt: number;
    kind: string;
    system: Array<{ path: string; enabled: boolean }>;
    user: Array<{ path: string; enabled: boolean }>;
    env: Array<{
      hive: string;
      name: string;
      kind: string;
      value: string | null;
      revision: string;
      sensitive: boolean;
      restorable: boolean;
    }>;
  } | null;
  /** 置 true 时 `history_list_events` 抛 `database` 错误（降级模式）。 */
  __historyDegraded?: boolean;
  /** 配置列表 fixture（P0-1 方案 A 的 DB 版）。 */
  __profilesFixture?: Array<{
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
  }>;
  /** `profile_paths` 的 fixture。 */
  __profilePathsFixture?: {
    name: string;
    updatedAt: number;
    system: Array<{ path: string; enabled: boolean }>;
    user: Array<{ path: string; enabled: boolean }>;
  } | null;
}
