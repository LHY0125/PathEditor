import { TargetType } from './undo-redo';

export type TabId = 'system' | 'user' | 'merged';

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

/** 判断某个 hive 是否可写；未初始化时回退到旧的 isAdmin 字段。 */
export function canWriteTarget(
  isAdmin: boolean,
  caps: PathCapabilities,
  target: TargetType,
): boolean {
  if (!hasCapabilities(caps)) return isAdmin;
  return target === TargetType.SYSTEM ? caps.canWriteSystem : caps.canWriteUser;
}
