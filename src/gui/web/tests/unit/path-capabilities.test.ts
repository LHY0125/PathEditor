import { describe, it, expect } from 'vitest';
import {
  canWriteTarget,
  EMPTY_CAPABILITIES,
  type PathCapabilities,
} from '@/core/path-capabilities';
import { TargetType } from '@/core/undo-redo';

/**
 * `canWriteTarget` 的两条语义：
 * 1. capabilities 有效时按 hive 独立判定（不得退化成全局 isAdmin）；
 * 2. capabilities 全空（IPC 失败）时回退 isAdmin —— 这是 UI 降级，
 *    不是安全判定；Rust 侧的注册表权限仍是唯一放行依据。
 */
describe('canWriteTarget', () => {
  it('capabilities 有效时按 hive 独立判定', () => {
    const caps: PathCapabilities = {
      canReadSystem: true,
      canWriteSystem: false,
      canReadUser: true,
      canWriteUser: true,
    };
    expect(canWriteTarget(true, caps, TargetType.SYSTEM)).toBe(false);
    expect(canWriteTarget(false, caps, TargetType.USER)).toBe(true);
  });

  it('capabilities 全空时回退 isAdmin（UI 降级）', () => {
    expect(canWriteTarget(true, EMPTY_CAPABILITIES, TargetType.SYSTEM)).toBe(true);
    expect(canWriteTarget(false, EMPTY_CAPABILITIES, TargetType.USER)).toBe(false);
  });
});
