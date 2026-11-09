import type { PathEntry } from '@/core/path-entry';

import { isRecord, parsePathEntries } from './parse-core';

// ── 历史契约 ─────────────────────────────────────────────

/** 事件动作（对应 core 的 `EventAction`，serde camelCase）。 */
export type EventAction =
  | 'save'
  | 'apply'
  | 'restore'
  | 'import'
  | 'envSet'
  | 'envAdd'
  | 'envRemove'
  | 'undo'
  | 'redo'
  | 'revealEnv'
  | 'drift';

/** 事件结果（对应 core 的 `EventResult`）。 */
export type EventResult = 'success' | 'partial' | 'failure';

/** 单条事件（对应 core 的 `EventView`）。 */
export interface EventView {
  uid: string;
  createdAt: number;
  source: string;
  action: EventAction;
  hive: string | null;
  target: string | null;
  result: EventResult;
  /** 本次操作产生的快照 uid；`null` 表示没有关联快照（无法查看详情）。 */
  snapshotUid: string | null;
}

/** 一页事件（`total` 是满足条件的总行数，不是本页条数）。 */
export interface EventPage {
  items: EventView[];
  total: number;
}

/** 事件列表筛选（覆盖 action/source/hive/target/时间范围）。 */
export interface EventFilter {
  limit?: number;
  offset?: number;
  action?: string;
  source?: string;
  /** `system` / `user` */
  hive?: string;
  /** 操作目标（变量名等） */
  target?: string;
  /** `success` / `partial` / `failure` */
  result?: string;
  /** 时间范围起点（含，Unix 毫秒） */
  fromMs?: number;
  /** 时间范围终点（含，Unix 毫秒） */
  toMs?: number;
}

/** 快照中的一行环境变量（对应 core 的 `EnvValueRow`）。 */
export interface HistoryEnvValue {
  hive: string;
  name: string;
  kind: string;
  /** 敏感值在 reveal=false 时为 `null`。 */
  value: string | null;
  revision: string;
  sensitive: boolean;
  restorable: boolean;
}

/** 快照视图（对应 core 的 `SnapshotView`）。 */
export interface SnapshotView {
  uid: string;
  createdAt: number;
  kind: string;
  system: PathEntry[];
  user: PathEntry[];
  env: HistoryEnvValue[];
  note: string | null;
  pinned: boolean;
}

/** 保留策略设置（对应 core 的 `RetentionSettings`）。 */
export interface RetentionSettings {
  snapshotKeepDays: number;
  snapshotKeepMax: number;
  eventKeepDays: number;
}

/** 校验 `RetentionSettings`。 */
export function parseRetentionSettings(value: unknown): RetentionSettings {
  if (!isRecord(value)) throw new Error('retention_settings 返回了无效契约');
  const { snapshotKeepDays, snapshotKeepMax, eventKeepDays } = value;
  if (
    typeof snapshotKeepDays !== 'number' ||
    typeof snapshotKeepMax !== 'number' ||
    typeof eventKeepDays !== 'number'
  ) {
    throw new Error('retention_settings 字段形状无效');
  }
  return { snapshotKeepDays, snapshotKeepMax, eventKeepDays };
}

/** 数据库健康快照（对应 core 的 `DbHealth`）。 */
export interface DbHealth {
  path: string;
  fileBytes: number;
  walBytes: number;
  shmBytes: number;
  schemaVersion: number;
  expectedSchemaVersion: number;
  integrity: string;
  integrityOk: boolean;
  tableRows: { name: string; rows: number }[];
  lastPruneAt: number | null;
}

/** 校验 `DbHealth`。 */
export function parseDbHealth(value: unknown): DbHealth {
  if (!isRecord(value)) throw new Error('db_health 返回了无效契约');
  const {
    path,
    fileBytes,
    walBytes,
    shmBytes,
    schemaVersion,
    expectedSchemaVersion,
    integrity,
    integrityOk,
    tableRows,
    lastPruneAt,
  } = value;
  if (
    typeof path !== 'string' ||
    typeof fileBytes !== 'number' ||
    typeof walBytes !== 'number' ||
    typeof shmBytes !== 'number' ||
    typeof schemaVersion !== 'number' ||
    typeof expectedSchemaVersion !== 'number' ||
    typeof integrity !== 'string' ||
    typeof integrityOk !== 'boolean' ||
    !Array.isArray(tableRows) ||
    (lastPruneAt !== null && typeof lastPruneAt !== 'number')
  ) {
    throw new Error('db_health 字段形状无效');
  }
  const rows = tableRows.map((row) => {
    if (!isRecord(row) || typeof row.name !== 'string' || typeof row.rows !== 'number') {
      throw new Error('db_health 的 tableRows 条目形状无效');
    }
    return { name: row.name, rows: row.rows };
  });
  return {
    path,
    fileBytes,
    walBytes,
    shmBytes,
    schemaVersion,
    expectedSchemaVersion,
    integrity,
    integrityOk,
    tableRows: rows,
    lastPruneAt,
  };
}

/** N 路对比的一行（对应 core 的 `MultiCompareEntry`）。 */
export interface MultiCompareEntry {
  baseUid: string;
  otherUid: string;
  diff: CompareResult;
}

/** 校验 `MultiCompareEntry[]`。 */
export function parseCompareMany(value: unknown): MultiCompareEntry[] {
  if (!Array.isArray(value)) throw new Error('compare_many 必须返回数组');
  return value.map((raw) => {
    if (!isRecord(raw) || typeof raw.baseUid !== 'string' || typeof raw.otherUid !== 'string') {
      throw new Error('compare_many 条目形状无效');
    }
    return {
      baseUid: raw.baseUid,
      otherUid: raw.otherUid,
      diff: parseCompareResult(raw.diff, 'compare_many.diff'),
    };
  });
}

/** 配置方案差异（对应 core 的 `ProfileDiff`）。 */
export interface ProfileDiff {
  left: string;
  right: string;
  leftDescription: string | null;
  rightDescription: string | null;
  tagsAdded: string[];
  tagsRemoved: string[];
  paths: CompareResult;
}

/** 校验 `ProfileDiff`。 */
export function parseProfileDiff(value: unknown): ProfileDiff {
  if (!isRecord(value)) throw new Error('profile_diff 返回了无效契约');
  const { left, right, leftDescription, rightDescription, tagsAdded, tagsRemoved } = value;
  if (
    typeof left !== 'string' ||
    typeof right !== 'string' ||
    (leftDescription !== null && typeof leftDescription !== 'string') ||
    (rightDescription !== null && typeof rightDescription !== 'string') ||
    !Array.isArray(tagsAdded) ||
    !tagsAdded.every((tag) => typeof tag === 'string') ||
    !Array.isArray(tagsRemoved) ||
    !tagsRemoved.every((tag) => typeof tag === 'string')
  ) {
    throw new Error('profile_diff 字段形状无效');
  }
  return {
    left,
    right,
    leftDescription,
    rightDescription,
    tagsAdded: tagsAdded as string[],
    tagsRemoved: tagsRemoved as string[],
    paths: parseCompareResult(value.paths, 'profile_diff.paths'),
  };
}

/** 配置方案视图（对应 core 的 `ProfileView`）。 */
export interface ProfileView {
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
}

const EVENT_ACTIONS: readonly EventAction[] = [
  'save',
  'apply',
  'restore',
  'import',
  'envSet',
  'envAdd',
  'envRemove',
  'undo',
  'redo',
  'revealEnv',
  'drift',
];
const EVENT_RESULTS: readonly EventResult[] = ['success', 'partial', 'failure'];

/** 校验 `EventPage` 契约；形状不合直接抛错，不做部分信任。 */
export function parseEventPage(value: unknown): EventPage {
  if (!isRecord(value) || !Array.isArray(value.items) || typeof value.total !== 'number') {
    throw new Error('history_list_events 返回了无效的分页契约');
  }
  const items = value.items.map((raw) => {
    if (!isRecord(raw)) throw new Error('事件项形状无效');
    const { uid, createdAt, source, hive, target } = raw;
    const action = raw.action;
    const result = raw.result;
    if (typeof uid !== 'string' || typeof createdAt !== 'number') {
      throw new Error('事件项 uid/createdAt 形状无效');
    }
    if (typeof source !== 'string') throw new Error('事件项 source 形状无效');
    if (typeof action !== 'string' || !EVENT_ACTIONS.includes(action as EventAction)) {
      throw new Error('事件项 action 非法');
    }
    if (typeof result !== 'string' || !EVENT_RESULTS.includes(result as EventResult)) {
      throw new Error('事件项 result 非法');
    }
    return {
      uid,
      createdAt,
      source,
      action: action as EventAction,
      hive: typeof hive === 'string' ? hive : null,
      target: typeof target === 'string' ? target : null,
      result: result as EventResult,
      snapshotUid: typeof raw.snapshotUid === 'string' ? raw.snapshotUid : null,
    };
  });
  return { items, total: value.total };
}

/** 校验 `SnapshotView`；**不信任上游的 value 字段**，照契约原样收下后由 UI 决定掩码。 */
export function parseSnapshotView(value: unknown, label = 'history'): SnapshotView {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的快照契约`);
  const { uid, createdAt, kind } = value;
  if (typeof uid !== 'string' || typeof createdAt !== 'number' || typeof kind !== 'string') {
    throw new Error(`${label} 的快照 uid/createdAt/kind 形状无效`);
  }
  if (!Array.isArray(value.env)) throw new Error(`${label} 的 env 必须是数组`);
  const env = value.env.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.hive !== 'string' ||
      typeof raw.name !== 'string' ||
      typeof raw.kind !== 'string' ||
      typeof raw.revision !== 'string' ||
      typeof raw.sensitive !== 'boolean' ||
      typeof raw.restorable !== 'boolean'
    ) {
      throw new Error(`${label} 的 env 条目形状无效`);
    }
    const v = raw.value;
    if (v !== null && typeof v !== 'string') {
      throw new Error(`${label} 的 env 值必须是字符串或 null`);
    }
    return {
      hive: raw.hive,
      name: raw.name,
      kind: raw.kind,
      value: v,
      revision: raw.revision,
      sensitive: raw.sensitive,
      restorable: raw.restorable,
    };
  });
  // note/pinned 是 v5 后加字段：缺省按空备注/未固定处理（兼容旧契约）
  const note = value.note === undefined ? null : value.note;
  if (note !== null && typeof note !== 'string') {
    throw new Error(`${label} 的 note 必须是字符串或 null`);
  }
  const pinned = value.pinned === undefined ? false : value.pinned;
  if (typeof pinned !== 'boolean') {
    throw new Error(`${label} 的 pinned 必须是布尔值`);
  }
  return {
    uid,
    createdAt,
    kind,
    system: parsePathEntries(value.system, `${label}.system`),
    user: parsePathEntries(value.user, `${label}.user`),
    env,
    note,
    pinned,
  };
}

/** 校验 `ProfileView[]`。 */
export function parseProfileList(value: unknown, label: string): ProfileView[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须是数组`);
  return value.map((raw) => {
    if (!isRecord(raw)) throw new Error(`${label} 的条目形状无效`);
    const { uid, name, description, tags, createdAt, updatedAt, lastAppliedAt } = raw;
    if (
      typeof uid !== 'string' ||
      typeof name !== 'string' ||
      typeof createdAt !== 'number' ||
      typeof updatedAt !== 'number' ||
      typeof raw.applyCount !== 'number' ||
      typeof raw.version !== 'number'
    ) {
      throw new Error(`${label} 的条目字段形状无效`);
    }
    if (!Array.isArray(tags) || !tags.every((t) => typeof t === 'string')) {
      throw new Error(`${label} 的 tags 必须是字符串数组`);
    }
    if (description !== null && typeof description !== 'string') {
      throw new Error(`${label} 的 description 必须是字符串或 null`);
    }
    if (lastAppliedAt !== null && typeof lastAppliedAt !== 'number') {
      throw new Error(`${label} 的 lastAppliedAt 必须是数字或 null`);
    }
    const snapshotUid = raw.snapshotUid;
    if (snapshotUid !== null && typeof snapshotUid !== 'string') {
      throw new Error(`${label} 的 snapshotUid 必须是字符串或 null`);
    }
    return {
      uid,
      name,
      description,
      tags: tags as string[],
      createdAt,
      updatedAt,
      lastAppliedAt,
      applyCount: raw.applyCount,
      version: raw.version,
      snapshotUid,
    };
  });
}

/* ────────────────────────── 检索 / 统计 / 对比 / 引用图 ────────────────────────── */

/** 一条搜索命中（对应 core 的 `db::search::SearchHit`）。 */
export interface SearchHit {
  /** 来源类型：`path` / `envName` / `profile` */
  kind: string;
  /** 指向对象（快照 uid 带 `#hive` 后缀，或配置 uid） */
  refUid: string;
  /** 完整索引内容（路径串 / 变量名 / 配置名+描述+标签）；不含环境变量值 */
  content: string;
  /** 高亮片段（FTS5 `snippet()`，命中处用 `[]` 包裹） */
  snippet: string;
  /** 关联实体时间（Unix 毫秒；旧契约缺省为 0） */
  createdAt: number;
  /** 排名，越小越相关 */
  rank: number;
}

/** 单日统计点。 */
export interface DailyPoint {
  /** `YYYY-MM-DD` */
  day: string;
  pathItems: number;
  envItems: number;
  events: number;
  conflicts: number;
}

/** 最常改变的变量。 */
export interface TargetCount {
  target: string;
  count: number;
}

/** 配置应用统计。 */
export interface ProfileApply {
  name: string;
  applyCount: number;
  lastAppliedAt: number | null;
}

/**
 * 统计面板数据。
 *
 * **口径**：`days` 是**历史**序列（某天的 `pathItems` 是「那天最后一次保存时的
 * 条目数」）；`currentPathItems` / `currentAvgLen` 是**现在**的状态。
 * UI 必须分开显示，否则用户会把「今天最后一次保存」误读成「当前」。
 */
export interface Dashboard {
  days: DailyPoint[];
  topTargets: TargetCount[];
  profileApplies: ProfileApply[];
  currentAvgLen: number;
  currentPathItems: number;
  scanRuns: number;
  conflictRate: number;
  topConflicts: TargetCount[];
}

/** 顺序变化的一条。 */
export interface ReorderEntry {
  path: string;
  leftIndex: number;
  rightIndex: number;
}

/** 启用状态变化的一条。 */
export interface ToggleEntry {
  path: string;
  leftEnabled: boolean;
  rightEnabled: boolean;
}

/** 单侧（system 或 user）的 PATH 差异。 */
export interface PathDiff {
  added: PathEntry[];
  removed: PathEntry[];
  reordered: ReorderEntry[];
  toggled: ToggleEntry[];
}

/**
 * 对比结果（system 与 user **分开**）。
 *
 * `envAdded/envRemoved/envModified` 只含变量名；`envDetails` 带 hive，
 * 用于区分同名变量。敏感值不参与对比结果。
 */
export interface EnvNameChange {
  hive: string;
  name: string;
  change: 'added' | 'removed' | 'modified';
}

export interface CompareResult {
  system: PathDiff;
  user: PathDiff;
  envAdded: string[];
  envRemoved: string[];
  envModified: string[];
  envDetails: EnvNameChange[];
}

/** 一条引用边。 */
export interface RefEdge {
  fromHive: string;
  fromName: string;
  toName: string;
  depth: number;
}

const isStringArray = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((x) => typeof x === 'string');

/** 校验 `SearchHit[]`。 */
export function parseSearchHits(value: unknown, label = 'search'): SearchHit[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.kind !== 'string' ||
      typeof raw.refUid !== 'string' ||
      typeof raw.content !== 'string' ||
      typeof raw.snippet !== 'string' ||
      typeof raw.rank !== 'number' ||
      (raw.createdAt !== undefined && typeof raw.createdAt !== 'number')
    ) {
      throw new Error(`${label} 的命中项形状无效`);
    }
    return {
      kind: raw.kind,
      refUid: raw.refUid,
      content: raw.content,
      snippet: raw.snippet,
      rank: raw.rank,
      createdAt: raw.createdAt ?? 0,
    };
  });
}

/** 校验 `Dashboard`。 */
export function parseDashboard(value: unknown, label = 'stats_dashboard'): Dashboard {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的统计契约`);
  if (!Array.isArray(value.days)) throw new Error(`${label} 的 days 必须是数组`);
  const days = value.days.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.day !== 'string' ||
      typeof raw.pathItems !== 'number' ||
      typeof raw.envItems !== 'number' ||
      typeof raw.events !== 'number' ||
      typeof raw.conflicts !== 'number'
    ) {
      throw new Error(`${label} 的 days 条目形状无效`);
    }
    return {
      day: raw.day,
      pathItems: raw.pathItems,
      envItems: raw.envItems,
      events: raw.events,
      conflicts: raw.conflicts,
    };
  });
  if (!Array.isArray(value.topTargets)) throw new Error(`${label} 的 topTargets 必须是数组`);
  const topTargets = value.topTargets.map((raw) => {
    if (!isRecord(raw) || typeof raw.target !== 'string' || typeof raw.count !== 'number') {
      throw new Error(`${label} 的 topTargets 条目形状无效`);
    }
    return { target: raw.target, count: raw.count };
  });
  if (!Array.isArray(value.profileApplies)) {
    throw new Error(`${label} 的 profileApplies 必须是数组`);
  }
  const profileApplies = value.profileApplies.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.name !== 'string' ||
      typeof raw.applyCount !== 'number' ||
      (raw.lastAppliedAt !== null && typeof raw.lastAppliedAt !== 'number')
    ) {
      throw new Error(`${label} 的 profileApplies 条目形状无效`);
    }
    return {
      name: raw.name,
      applyCount: raw.applyCount,
      lastAppliedAt: raw.lastAppliedAt,
    };
  });
  if (typeof value.currentAvgLen !== 'number' || typeof value.currentPathItems !== 'number') {
    throw new Error(`${label} 的 current* 字段必须是数字`);
  }
  // 冲突统计是后加字段：缺省按零值处理（兼容旧契约）
  const scanRuns = value.scanRuns === undefined ? 0 : value.scanRuns;
  const conflictRate = value.conflictRate === undefined ? 0 : value.conflictRate;
  if (typeof scanRuns !== 'number' || typeof conflictRate !== 'number') {
    throw new Error(`${label} 的冲突统计字段必须是数字`);
  }
  const topConflictsRaw = value.topConflicts === undefined ? [] : value.topConflicts;
  if (!Array.isArray(topConflictsRaw)) {
    throw new Error(`${label} 的 topConflicts 必须是数组`);
  }
  const topConflicts = topConflictsRaw.map((raw) => {
    if (!isRecord(raw) || typeof raw.target !== 'string' || typeof raw.count !== 'number') {
      throw new Error(`${label} 的 topConflicts 条目形状无效`);
    }
    return { target: raw.target, count: raw.count };
  });
  return {
    days,
    topTargets,
    profileApplies,
    currentAvgLen: value.currentAvgLen,
    currentPathItems: value.currentPathItems,
    scanRuns,
    conflictRate,
    topConflicts,
  };
}

/** 校验单侧 `PathDiff`。 */
export function parsePathDiff(value: unknown, label: string): PathDiff {
  if (!isRecord(value)) throw new Error(`${label} 必须是对象`);
  if (!Array.isArray(value.reordered)) throw new Error(`${label}.reordered 必须是数组`);
  if (!Array.isArray(value.toggled)) throw new Error(`${label}.toggled 必须是数组`);
  const reordered = value.reordered.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.path !== 'string' ||
      typeof raw.leftIndex !== 'number' ||
      typeof raw.rightIndex !== 'number'
    ) {
      throw new Error(`${label}.reordered 条目形状无效`);
    }
    return { path: raw.path, leftIndex: raw.leftIndex, rightIndex: raw.rightIndex };
  });
  const toggled = value.toggled.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.path !== 'string' ||
      typeof raw.leftEnabled !== 'boolean' ||
      typeof raw.rightEnabled !== 'boolean'
    ) {
      throw new Error(`${label}.toggled 条目形状无效`);
    }
    return {
      path: raw.path,
      leftEnabled: raw.leftEnabled,
      rightEnabled: raw.rightEnabled,
    };
  });
  return {
    added: parsePathEntries(value.added, `${label}.added`),
    removed: parsePathEntries(value.removed, `${label}.removed`),
    reordered,
    toggled,
  };
}

/** 校验 `CompareResult`（双 hive 都必须存在）。 */
export function parseCompareResult(value: unknown, label = 'compare_snapshots'): CompareResult {
  if (!isRecord(value)) throw new Error(`${label} 返回了无效的对比契约`);
  // 缺 system 或 user 一律报错：只比一侧会让「只有用户 PATH 变了」看起来像没有差异，
  // 不能让后端退回旧形状而前端静默接受
  if (value.system === undefined || value.user === undefined) {
    throw new Error(`${label} 必须同时返回 system 与 user 两侧`);
  }
  // 逐个校验并**收窄类型**：TS 不会把循环内守卫的结果带到 return，
  // 因此这里先取回局部变量、逐个断言，再放进返回值。
  const envAdded = value.envAdded;
  const envRemoved = value.envRemoved;
  const envModified = value.envModified;
  if (!isStringArray(envAdded)) throw new Error(`${label}.envAdded 必须是字符串数组`);
  if (!isStringArray(envRemoved)) throw new Error(`${label}.envRemoved 必须是字符串数组`);
  if (!isStringArray(envModified)) throw new Error(`${label}.envModified 必须是字符串数组`);
  // envDetails 是后加的字段：缺省按空数组处理（兼容旧契约），
  // 出现了就必须是完整形状。
  const envDetails =
    value.envDetails === undefined ? [] : parseEnvNameChanges(value.envDetails, label);
  return {
    system: parsePathDiff(value.system, `${label}.system`),
    user: parsePathDiff(value.user, `${label}.user`),
    envAdded,
    envRemoved,
    envModified,
    envDetails,
  };
}

/** 校验 `EnvNameChange[]`。 */
function parseEnvNameChanges(value: unknown, label: string): EnvNameChange[] {
  if (!Array.isArray(value)) throw new Error(`${label}.envDetails 必须是数组`);
  return value.map((item) => {
    if (
      !isRecord(item) ||
      typeof item.hive !== 'string' ||
      typeof item.name !== 'string' ||
      (item.change !== 'added' && item.change !== 'removed' && item.change !== 'modified')
    ) {
      throw new Error(`${label}.envDetails 返回了无效条目`);
    }
    return { hive: item.hive, name: item.name, change: item.change };
  });
}

/** 校验 `RefEdge[]`。 */
export function parseRefEdges(value: unknown, label = 'refs'): RefEdge[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.fromHive !== 'string' ||
      typeof raw.fromName !== 'string' ||
      typeof raw.toName !== 'string' ||
      typeof raw.depth !== 'number'
    ) {
      throw new Error(`${label} 的引用边形状无效`);
    }
    return {
      fromHive: raw.fromHive,
      fromName: raw.fromName,
      toName: raw.toName,
      depth: raw.depth,
    };
  });
}

/** 校验 `string[][]`（引用环）。 */
export function parseCycleList(value: unknown, label = 'refs_cycles'): string[][] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (!isStringArray(raw)) throw new Error(`${label} 的每个环必须是字符串数组`);
    return raw;
  });
}

/* ────────────────────────── 同步中心 ────────────────────────── */

/** 导出结果（`core::db::sync::ExportReport`）。 */
export interface SyncExportReport {
  /** 本次导出的变更条数 */
  changeCount: number;
  /** 写出的文件路径 */
  path: string;
}

/** 导入结果（`core::db::sync::ImportReport`）。 */
export interface SyncImportReport {
  /** 实际应用的变更数（真正落库的条数） */
  applied: number;
  /** 因已存在而跳过的变更数（幂等） */
  skipped: number;
  /** 记录的冲突数 */
  conflicts: number;
  /**
   * 本版本还不支持落库、因而**未应用也未记录**的变更数。
   *
   * 界面必须显示这个数字：否则用户会把「全部同步成功」当成事实。
   */
  skippedUnsupported: number;
  /**
   * 因与本地配置同名而**自动改名**的条数。
   *
   * 改名只影响本地行；不显示的话，用户会发现配置名字变了却找不到原因。
   */
  renamedProfiles: number;
}

/** 同步中心首屏状态。 */
export interface SyncStatus {
  /** 待导出变更数 */
  pendingCount: number;
  /** 未解决冲突数 */
  unresolvedConflicts: number;
}

/** 冲突的展示形状（`core::db::sync::conflict::SyncConflictView`）。 */
export interface SyncConflict {
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
}

/** 校验 `SyncExportReport`。 */
export function parseSyncExportReport(value: unknown, label = 'sync_export'): SyncExportReport {
  if (!isRecord(value) || typeof value.changeCount !== 'number' || typeof value.path !== 'string') {
    throw new Error(`${label} 返回了无效的导出结果`);
  }
  return { changeCount: value.changeCount, path: value.path };
}

/** 校验 `SyncImportReport`。 */
export function parseSyncImportReport(value: unknown, label = 'sync_import'): SyncImportReport {
  if (
    !isRecord(value) ||
    typeof value.applied !== 'number' ||
    typeof value.skipped !== 'number' ||
    typeof value.conflicts !== 'number' ||
    typeof value.skippedUnsupported !== 'number' ||
    typeof value.renamedProfiles !== 'number'
  ) {
    throw new Error(`${label} 返回了无效的导入结果`);
  }
  return {
    applied: value.applied,
    skipped: value.skipped,
    conflicts: value.conflicts,
    skippedUnsupported: value.skippedUnsupported,
    renamedProfiles: value.renamedProfiles,
  };
}

/** 校验 `SyncStatus`。 */
export function parseSyncStatus(value: unknown, label = 'sync_status'): SyncStatus {
  if (
    !isRecord(value) ||
    typeof value.pendingCount !== 'number' ||
    typeof value.unresolvedConflicts !== 'number'
  ) {
    throw new Error(`${label} 返回了无效的同步状态`);
  }
  return {
    pendingCount: value.pendingCount,
    unresolvedConflicts: value.unresolvedConflicts,
  };
}

/** 校验 `SyncConflict[]`。 */
export function parseSyncConflicts(value: unknown, label = 'sync_list_conflicts'): SyncConflict[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须返回数组`);
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.id !== 'number' ||
      typeof raw.entityType !== 'string' ||
      typeof raw.entityUid !== 'string' ||
      (raw.field !== null && typeof raw.field !== 'string') ||
      (raw.localValue !== null && typeof raw.localValue !== 'string') ||
      (raw.remoteValue !== null && typeof raw.remoteValue !== 'string') ||
      typeof raw.remoteDevice !== 'string' ||
      typeof raw.createdAt !== 'number' ||
      (raw.resolvedAt !== null && typeof raw.resolvedAt !== 'number') ||
      (raw.resolution !== null && typeof raw.resolution !== 'string')
    ) {
      throw new Error(`${label} 的冲突条目形状无效`);
    }
    return {
      id: raw.id,
      entityType: raw.entityType,
      entityUid: raw.entityUid,
      field: raw.field,
      localValue: raw.localValue,
      remoteValue: raw.remoteValue,
      remoteDevice: raw.remoteDevice,
      createdAt: raw.createdAt,
      resolvedAt: raw.resolvedAt,
      resolution: raw.resolution,
    };
  });
}

/** 单条冲突详情（含远端原始内容）。 */
export interface SyncConflictDetail {
  conflict: SyncConflict;
  remoteContent: string | null;
}

/** 校验 `SyncConflictDetail`。 */
export function parseSyncConflictDetail(value: unknown): SyncConflictDetail {
  if (!isRecord(value) || !('conflict' in value)) {
    throw new Error('sync_conflict_detail 返回了无效契约');
  }
  const [conflict] = parseSyncConflicts([value.conflict], 'sync_conflict_detail.conflict');
  if (value.remoteContent !== null && typeof value.remoteContent !== 'string') {
    throw new Error('sync_conflict_detail.remoteContent 必须是字符串或 null');
  }
  return { conflict, remoteContent: value.remoteContent };
}

/** 规则条件/动作（对应 core 的 `RuleCondition` / `RuleAction`）。 */
export type RuleCondition = { kind: 'pathContains'; value: string };
export type RuleAction = 'moveTop' | 'remove';

/** 一条整理规则。 */
export interface Rule {
  uid: string;
  name: string;
  enabled: boolean;
  priority: number;
  condition: RuleCondition;
  action: RuleAction;
  createdAt: number;
  updatedAt: number;
}

/** 规则执行结果。 */
export interface RuleApplyResult {
  system: PathEntry[];
  user: PathEntry[];
  hits: { ruleUid: string; matchedCount: number }[];
}

export function parseRules(value: unknown): Rule[] {
  if (!Array.isArray(value)) throw new Error('rules_list 必须返回数组');
  return value.map((raw) => {
    if (
      !isRecord(raw) ||
      typeof raw.uid !== 'string' ||
      typeof raw.name !== 'string' ||
      typeof raw.enabled !== 'boolean' ||
      typeof raw.priority !== 'number' ||
      typeof raw.createdAt !== 'number' ||
      typeof raw.updatedAt !== 'number' ||
      (raw.action !== 'moveTop' && raw.action !== 'remove') ||
      !isRecord(raw.condition) ||
      raw.condition.kind !== 'pathContains' ||
      typeof raw.condition.value !== 'string'
    ) {
      throw new Error('rules_list 条目形状无效');
    }
    return {
      uid: raw.uid,
      name: raw.name,
      enabled: raw.enabled,
      priority: raw.priority,
      condition: { kind: 'pathContains', value: raw.condition.value },
      action: raw.action,
      createdAt: raw.createdAt,
      updatedAt: raw.updatedAt,
    };
  });
}

export function parseRuleApplyResult(value: unknown): RuleApplyResult {
  if (!isRecord(value)) throw new Error('rules_apply 返回了无效契约');
  if (!Array.isArray(value.hits)) throw new Error('rules_apply.hits 必须是数组');
  const hits = value.hits.map((raw) => {
    if (!isRecord(raw) || typeof raw.ruleUid !== 'string' || typeof raw.matchedCount !== 'number') {
      throw new Error('rules_apply.hits 条目形状无效');
    }
    return { ruleUid: raw.ruleUid, matchedCount: raw.matchedCount };
  });
  return {
    system: parsePathEntries(value.system, 'rules_apply.system'),
    user: parsePathEntries(value.user, 'rules_apply.user'),
    hits,
  };
}
