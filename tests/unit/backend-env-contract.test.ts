import { describe, it, expect, vi, beforeEach } from 'vitest';

// backend.ts 的 EnvVarMeta 运行时校验（F-05）：白名单复制、拒绝 value 字段、
// 非空 name/revision。直接 mock @tauri-apps/api/core 的 invoke 注入恶意返回值。
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { backend } from '@/services/backend';
import type { CoreError, EnvVarMeta } from '@/core/env-var';

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

describe('parseCoreError rejection 解析（F-06 双形状兼容）', () => {
  it('结构化 CoreError 对象（合法 code）原样结构化透传', async () => {
    const rejection = {
      code: 'conflict',
      operation: 'update_env_var',
      hive: 'user',
      name: 'JAVA_HOME',
      retryable: true,
      message: '[E_CONFLICT] 变量已被其他进程修改，请重新加载',
    };
    mockInvoke.mockRejectedValue(rejection);

    await expect(backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1')).rejects.toMatchObject({
      code: 'conflict',
      message: rejection.message,
    });
  });

  it('纯字符串 rejection（PATH 通路）兜底为 internal + 原文', async () => {
    mockInvoke.mockRejectedValue('无法写入系统注册表（需要管理员权限）');

    const err = await backend.updateEnvVar('user', 'JAVA_HOME', 'v', 'rev-1').catch((e) => e);

    const core = err as CoreError;
    expect(core.code).toBe('internal');
    expect(core.message).toBe('无法写入系统注册表（需要管理员权限）');
  });

  it('Error 对象 rejection 兜底为 internal，message 取 String(error)', async () => {
    mockInvoke.mockRejectedValue(new Error('boom'));

    const err = await backend.deleteEnvVar('user', 'X', 'rev').catch((e) => e);

    expect((err as CoreError).code).toBe('internal');
    expect((err as CoreError).message).toContain('boom');
  });

  it('对象 rejection 但 code 非法时降级为 internal，不透传来路不明字段', async () => {
    mockInvoke.mockRejectedValue({ code: 'hacker', message: '伪造', extra: 'payload' });

    const err = await backend.updateEnvVar('user', 'X', 'v', 'rev').catch((e) => e);

    expect((err as CoreError).code).toBe('internal');
    expect(Object.keys(err as object).sort()).toEqual(['code', 'message']);
  });

  it('对象 rejection 缺 message 时降级为 internal', async () => {
    mockInvoke.mockRejectedValue({ code: 'conflict' });

    const err = await backend.updateEnvVar('user', 'X', 'v', 'rev').catch((e) => e);

    expect((err as CoreError).code).toBe('internal');
  });

  it('未知 code 对象但带非空 message 时兜底为 internal 且保留原文', async () => {
    // 兜底不得丢 message：String(error) 会变成 "[object Object]"（终审 M-1）
    mockInvoke.mockRejectedValue({ code: 'unknown_kind', message: '未知错误详情' });

    const err = await backend.updateEnvVar('user', 'X', 'v', 'rev').catch((e) => e);

    expect((err as CoreError).code).toBe('internal');
    expect((err as CoreError).message).toBe('未知错误详情');
  });

  it('成功时原样返回，不包裹', async () => {
    mockInvoke.mockResolvedValue(undefined);

    await expect(backend.updateEnvVar('user', 'X', 'v', 'rev')).resolves.toBeUndefined();
  });
});

describe('revealEnvVar 的 RevealedValue 契约校验（F-01）', () => {
  it('合法返回值（value + revision 均为非空 string）通过并白名单构造', async () => {
    mockInvoke.mockResolvedValue({ value: 'C:\\Java\\bin', revision: 'rev-9' });

    const revealed = await backend.revealEnvVar('user', 'JAVA_HOME');

    expect(revealed).toEqual({ value: 'C:\\Java\\bin', revision: 'rev-9' });
    // 白名单构造：结果对象上没有多余字段
    expect(Object.keys(revealed).sort()).toEqual(['revision', 'value']);
  });

  it('返回值不是 record 时拒绝', async () => {
    mockInvoke.mockResolvedValue('plain-string');

    await expect(backend.revealEnvVar('user', 'JAVA_HOME')).rejects.toThrow(/RevealedValue/);
  });

  it('value 为非 string 时拒绝', async () => {
    mockInvoke.mockResolvedValue({ value: 42, revision: 'rev-9' });

    await expect(backend.revealEnvVar('user', 'JAVA_HOME')).rejects.toThrow(/RevealedValue/);
  });

  it('revision 为非 string 时拒绝', async () => {
    mockInvoke.mockResolvedValue({ value: 'C:\\Java\\bin', revision: 123 });

    await expect(backend.revealEnvVar('user', 'JAVA_HOME')).rejects.toThrow(/RevealedValue/);
  });

  it('revision 为空字符串时拒绝', async () => {
    mockInvoke.mockResolvedValue({ value: 'C:\\Java\\bin', revision: '' });

    await expect(backend.revealEnvVar('user', 'JAVA_HOME')).rejects.toThrow(/RevealedValue/);
  });

  it('携带多余字段（如 hive）时白名单丢弃，不透传', async () => {
    mockInvoke.mockResolvedValue({ value: 'v', revision: 'r', hive: 'user', sensitive: true });

    const revealed = await backend.revealEnvVar('user', 'JAVA_HOME');

    expect(revealed).toEqual({ value: 'v', revision: 'r' });
  });
});

describe('parseEnvVarSnapshot 的 capturedAt 兼容回退（F-05 向后兼容）', () => {
  it('capturedAt 缺失时回退为 0', async () => {
    mockInvoke.mockResolvedValue({ system: [validMeta()], user: [] });

    const snapshot = await backend.listAllEnvVars();

    expect(snapshot.capturedAt).toBe(0);
  });

  it('capturedAt 为非 number（如 string）时回退为 0', async () => {
    mockInvoke.mockResolvedValue({ system: [], user: [], capturedAt: 'not-a-number' });

    const snapshot = await backend.listAllEnvVars();

    expect(snapshot.capturedAt).toBe(0);
  });

  it('capturedAt 为合法 number 时原样保留', async () => {
    mockInvoke.mockResolvedValue({ system: [], user: [], capturedAt: 1726900000000 });

    const snapshot = await backend.listAllEnvVars();

    expect(snapshot.capturedAt).toBe(1726900000000);
  });
});
