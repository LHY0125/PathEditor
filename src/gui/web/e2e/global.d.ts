interface Window {
  __TAURI_INTERNALS__?: {
    invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  };
  /** 历史相关 mock 的注入点。每个用例按需设置。 */
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
  /** 配置列表 fixture。 */
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

  /* ── ────────────────────────────────────── */

  /** 置 true 时 `search` / `stats_dashboard` 抛 `database` 错误（降级模式）。 */
  __m2Degraded?: boolean;
  /** `search` 的返回 fixture（`SearchHit[]`）。 */
  __searchFixture?: Array<{
    kind: string;
    refUid: string;
    snippet: string;
    rank: number;
  }>;
  /** `stats_dashboard` 的返回 fixture（形状必须与 Rust 序列化一致）。 */
  __statsFixture?: {
    days: Array<{
      day: string;
      pathItems: number;
      envItems: number;
      events: number;
      conflicts: number;
    }>;
    topTargets: Array<{ target: string; count: number }>;
    profileApplies: Array<{
      name: string;
      applyCount: number;
      lastAppliedAt: number | null;
    }>;
    currentAvgLen: number;
    currentPathItems: number;
  };
  /** `compare_snapshots` 的返回 fixture。 */
  __compareFixture?: {
    system: {
      added: Array<{ path: string; enabled: boolean }>;
      removed: Array<{ path: string; enabled: boolean }>;
      reordered: Array<{ path: string; leftIndex: number; rightIndex: number }>;
      toggled: Array<{ path: string; leftEnabled: boolean; rightEnabled: boolean }>;
    };
    user: {
      added: Array<{ path: string; enabled: boolean }>;
      removed: Array<{ path: string; enabled: boolean }>;
      reordered: Array<{ path: string; leftIndex: number; rightIndex: number }>;
      toggled: Array<{ path: string; leftEnabled: boolean; rightEnabled: boolean }>;
    };
    envAdded: string[];
    envRemoved: string[];
    envModified: string[];
  };
  /** 引用图三个方向的 fixture。 */
  __refsFixture?: {
    dependents: Array<{
      fromHive: string;
      fromName: string;
      toName: string;
      depth: number;
    }>;
    dependencies: Array<{
      fromHive: string;
      fromName: string;
      toName: string;
      depth: number;
    }>;
    cycles: string[][];
  };
  /** `sync_status` 的返回 fixture。 */
  __syncStatusFixture?: {
    pendingCount: number;
    unresolvedConflicts: number;
  };
  /** `sync_list_conflicts` 的返回 fixture。 */
  __syncConflictsFixture?: Array<{
    id: number;
    entityType: string;
    entityUid: string;
    field: string | null;
    localValue: string | null;
    remoteValue: string | null;
    remoteDevice: string;
    createdAt: number;
    resolvedAt: number | null;
    resolution: string | null;
  }>;
  /** 置 true 时 `sync_export` / `sync_import` 抛 `crypto` 错误（错误口令路径）。 */
  __syncCryptoError?: boolean;
}
