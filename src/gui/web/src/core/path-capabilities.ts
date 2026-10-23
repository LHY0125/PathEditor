import { TargetType } from './undo-redo';

export type TabId =
  | 'system'
  | 'user'
  | 'allVars'
  | 'merged'
  | 'history'
  // 在既有联合类型里追加，不是新建类型；`app-store.ts` 已 re-export 本类型。
  | 'search'
  | 'stats'
  | 'sync';

export interface PathCapabilities {
  canReadSystem: boolean;
  canWriteSystem: boolean;
  canReadUser: boolean;
  canWriteUser: boolean;
}

export const EMPTY_CAPABILITIES: PathCapabilities = {
  canReadSystem: false,
  canWriteSystem: false,
  canReadUser: false,
  canWriteUser: false,
};

function hasCapabilities(caps: PathCapabilities): boolean {
  return caps.canReadSystem || caps.canWriteSystem || caps.canReadUser || caps.canWriteUser;
}

/** 当前 tab 对应的目标 hive；merged 视图没有单一写入目标。 */
export function targetForTab(tab: TabId): TargetType | null {
  if (tab === 'system') return TargetType.SYSTEM;
  if (tab === 'user') return TargetType.USER;
  return null;
}

/** 判断某个 hive 是否可写；未初始化时回退到 `isAdmin`。
 *
 * **这是 UI 降级，不是安全判定**：capabilities 加载失败（IPC 不可用）时，
 * 用 `isAdmin` 兜底以免整个界面变成只读。真正决定写不写得了的是 Rust 侧的
 * 注册表权限与 `CoreError`；前端不得据此放行任何 core 会拒绝的操作。
 */
export function canWriteTarget(
  isAdmin: boolean,
  caps: PathCapabilities,
  target: TargetType,
): boolean {
  if (!hasCapabilities(caps)) return isAdmin;
  return target === TargetType.SYSTEM ? caps.canWriteSystem : caps.canWriteUser;
}
