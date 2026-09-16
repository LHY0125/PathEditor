import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  return {
    backend: {
      listAllEnvVars: viModule.fn(),
      revealEnvVar: viModule.fn(),
      updateEnvVar: viModule.fn(),
      createEnvVar: viModule.fn(),
      deleteEnvVar: viModule.fn(),
    },
  };
});

import { useEnvStore } from '@/store/env-store';
import { backend } from '@/services/backend';
import type { EnvVarMeta } from '@/core/env-var';

const mockBackend = vi.mocked(backend);

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
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

const snapshot = {
  system: [meta({ name: 'windir', hive: 'system' })],
  user: [
    meta({ name: 'JAVA_HOME', hive: 'user' }),
    meta({ name: 'MY_TOKEN', hive: 'user', sensitive: true, preview: null }),
  ],
};

beforeEach(() => {
  // resetAllMocks 而非 clearAllMocks：后者只清调用记录，会留下上一个用例设置的
  // mockResolvedValue / mockRejectedValue 实现，造成用例间顺序耦合。
  vi.resetAllMocks();
  useEnvStore.setState({
    snapshot: null,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',
  });
});

describe('load', () => {
  it('载入两个 hive 的快照并清除全部明文', async () => {
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    useEnvStore.setState({ revealed: new Map([['user:MY_TOKEN', 'leaked']]) });

    await useEnvStore.getState().load();

    const state = useEnvStore.getState();
    expect(state.snapshot).toEqual(snapshot);
    expect(state.revealed.size).toBe(0);
    expect(state.isLoading).toBe(false);
  });

  it('载入失败时写 statusMessage 且不抛异常', async () => {
    mockBackend.listAllEnvVars.mockRejectedValue(new Error('boom'));

    await expect(useEnvStore.getState().load()).resolves.toBeUndefined();

    expect(useEnvStore.getState().statusMessage).toContain('boom');
    expect(useEnvStore.getState().isLoading).toBe(false);
  });
});

describe('reveal / hide', () => {
  it('reveal 存入明文，hide 清除', async () => {
    mockBackend.revealEnvVar.mockResolvedValue('real-secret');
    const target = meta({ name: 'MY_TOKEN', sensitive: true, preview: null });

    await useEnvStore.getState().reveal(target);
    expect(useEnvStore.getState().revealed.get('user:MY_TOKEN')).toBe('real-secret');
    expect(mockBackend.revealEnvVar).toHaveBeenCalledWith('user', 'MY_TOKEN');

    useEnvStore.getState().hide(target);
    expect(useEnvStore.getState().revealed.has('user:MY_TOKEN')).toBe(false);
  });

  it('reveal 失败时触发刷新并清除明文', async () => {
    mockBackend.revealEnvVar.mockRejectedValue(new Error('类型不受支持'));
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta({ name: 'MY_TOKEN', sensitive: true, preview: null });

    await useEnvStore.getState().reveal(target);

    expect(useEnvStore.getState().revealed.has('user:MY_TOKEN')).toBe(false);
    expect(mockBackend.listAllEnvVars).toHaveBeenCalled();
  });
});

describe('save', () => {
  it('携带 meta.revision 调用 updateEnvVar', async () => {
    mockBackend.updateEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');

    await useEnvStore.getState().save(target);

    expect(mockBackend.updateEnvVar).toHaveBeenCalledWith(
      'user',
      'JAVA_HOME',
      'C:\\NewJava',
      'rev-1',
    );
    expect(useEnvStore.getState().draft.has('user:JAVA_HOME')).toBe(false);
  });

  it('revision 冲突时不重试、提示并刷新', async () => {
    mockBackend.updateEnvVar.mockRejectedValue(new Error('变量已被其他进程修改，请重新加载'));
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');

    await useEnvStore.getState().save(target);

    expect(mockBackend.updateEnvVar).toHaveBeenCalledTimes(1);
    expect(useEnvStore.getState().statusMessage).toContain('已被其他进程修改');
    expect(mockBackend.listAllEnvVars).toHaveBeenCalled();
    // 草稿保留，避免用户输入丢失
    expect(useEnvStore.getState().draft.has('user:JAVA_HOME')).toBe(true);
  });
});

describe('create / remove', () => {
  it('create 校验名称后调用 createEnvVar', async () => {
    mockBackend.createEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);

    await useEnvStore.getState().create('user', 'NEW_VAR', 'value', 'string');

    expect(mockBackend.createEnvVar).toHaveBeenCalledWith('user', 'NEW_VAR', 'value', 'string');
  });

  it('create 名称非法时直接拒绝，不调用 IPC', async () => {
    await useEnvStore.getState().create('user', 'BAD=NAME', 'value', 'string');

    expect(mockBackend.createEnvVar).not.toHaveBeenCalled();
    expect(useEnvStore.getState().statusMessage).not.toBe('');
  });

  it('remove 携带 meta.revision 调用 deleteEnvVar', async () => {
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();

    await useEnvStore.getState().remove(target);

    expect(mockBackend.deleteEnvVar).toHaveBeenCalledWith('user', 'JAVA_HOME', 'rev-1');
  });
});

describe('setHiveFilter', () => {
  it('切换筛选不触发 IPC', () => {
    useEnvStore.getState().setHiveFilter('system');

    expect(useEnvStore.getState().hiveFilter).toBe('system');
    expect(mockBackend.listAllEnvVars).not.toHaveBeenCalled();
  });
});

describe('操作结果返回值（弹窗据此决定是否关闭）', () => {
  it('save 成功返回 true，失败返回 false', async () => {
    const target = meta();

    mockBackend.updateEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');
    await expect(useEnvStore.getState().save(target)).resolves.toBe(true);

    mockBackend.updateEnvVar.mockRejectedValue(new Error('boom'));
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');
    await expect(useEnvStore.getState().save(target)).resolves.toBe(false);
  });

  it('create 成功返回 true，失败返回 false', async () => {
    mockBackend.createEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    await expect(useEnvStore.getState().create('user', 'N1', 'v', 'string')).resolves.toBe(true);

    mockBackend.createEnvVar.mockRejectedValue(new Error('变量已存在'));
    await expect(useEnvStore.getState().create('user', 'N2', 'v', 'string')).resolves.toBe(false);
  });

  it('remove 成功返回 true，失败返回 false', async () => {
    const target = meta();

    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    await expect(useEnvStore.getState().remove(target)).resolves.toBe(true);

    mockBackend.deleteEnvVar.mockRejectedValue(new Error('boom'));
    await expect(useEnvStore.getState().remove(target)).resolves.toBe(false);
  });

  it('无草稿时 save 直接返回 false', async () => {
    await expect(useEnvStore.getState().save(meta())).resolves.toBe(false);
    expect(mockBackend.updateEnvVar).not.toHaveBeenCalled();
  });
});
