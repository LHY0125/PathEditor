import { describe, it, expect, vi, beforeEach } from 'vitest';

// backend.ts 的 EnvVarMeta 运行时校验（F-05）：白名单复制、拒绝 value 字段、
// 非空 name/revision。直接 mock @tauri-apps/api/core 的 invoke 注入恶意返回值。
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { backend } from '@/services/backend';
import type { EnvVarMeta } from '@/core/env-var';

const mockInvoke = vi.mocked(invoke);

/** 构造一份契约合法的 EnvVarMeta（camelCase 序列化形态）。 */
function validMeta(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
});

describe('listAllEnvVars 运行时契约校验（F-05）', () => {
  it('合法返回值通过并按白名单复制 8 个字段', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta()], user: [] });

    const snapshot = await backend.listAllEnvVars();

    expect(snapshot.system).toHaveLength(1);
    const meta: EnvVarMeta = snapshot.system[0];
    expect(meta).toEqual(validMeta());
    // 白名单构造：结果对象上没有多余字段
    expect(Object.keys(meta).sort()).toEqual(
      ['canDelete', 'canEdit', 'hive', 'kind', 'name', 'preview', 'revision', 'sensitive'].sort(),
    );
  });

  it('携带 value 字段（敏感明文泄漏）时拒绝整个响应', async () => {
    mockInvoke.mockResolvedValue({
      system: [validMeta({ value: 'leaked-secret' })],
      user: [],
    });

    await expect(backend.listAllEnvVars()).rejects.toThrow(/value/);
  });

  it('空 name 被拒绝', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta({ name: '   ' })], user: [] });

    await expect(backend.listAllEnvVars()).rejects.toThrow(/name/);
  });

  it('空 revision 被拒绝', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta({ revision: '' })], user: [] });

    await expect(backend.listAllEnvVars()).rejects.toThrow(/revision/);
  });

  it('非法 kind / hive 被拒绝', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta({ kind: 'dword' })], user: [] });
    await expect(backend.listAllEnvVars()).rejects.toThrow(/kind/);

    mockInvoke.mockResolvedValue({ system: [validMeta({ hive: 'machine' })], user: [] });
    await expect(backend.listAllEnvVars()).rejects.toThrow(/hive/);
  });

  it('preview 为非 string/null 被拒绝', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta({ preview: 42 })], user: [] });

    await expect(backend.listAllEnvVars()).rejects.toThrow(/preview/);
  });
});
