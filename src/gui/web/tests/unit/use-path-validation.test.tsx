import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import type { PathEntry } from '@/core/path-entry';

vi.mock('@/services/backend', () => ({
  backend: {
    validatePath: vi.fn(),
    expandEnvVars: vi.fn(),
  },
}));

import { usePathValidation, VALIDATION_CONCURRENCY } from '@/hooks/use-path-validation';
import { backend } from '@/services/backend';

const validatePath = vi.mocked(backend.validatePath);
const expandEnvVars = vi.mocked(backend.expandEnvVars);

function entries(count: number, prefix = 'C:\\path'): PathEntry[] {
  return Array.from({ length: count }, (_, index) => ({
    path: `${prefix}${index}`,
    enabled: true,
  }));
}

describe('usePathValidation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    validatePath.mockResolvedValue(true);
    expandEnvVars.mockImplementation(
      async (path: string) => `C:\\expanded\\${path.replace(/%/g, '')}`,
    );
  });

  it('空列表不发起 IPC', () => {
    const { result } = renderHook(() => usePathValidation([]));
    expect(result.current.validationCache.size).toBe(0);
    expect(validatePath).not.toHaveBeenCalled();
  });

  it('单条路径完成验证', async () => {
    const { result } = renderHook(() => usePathValidation(entries(1)));
    await waitFor(() => expect(result.current.validationCache.get('C:\\path0')).toBe('valid'));
  });

  it.each([19, 20, 21, 100])('处理全部 %i 条路径，而不是只处理前 20 条', async (count) => {
    const { result } = renderHook(() => usePathValidation(entries(count)));
    await waitFor(() => expect(result.current.validationCache.size).toBe(count));
    expect([...result.current.validationCache.values()].every((state) => state === 'valid')).toBe(
      true,
    );
    expect(validatePath).toHaveBeenCalledTimes(count);
  });

  it('展开环境变量并验证展开后的目录', async () => {
    const paths: PathEntry[] = [{ path: '%JAVA_HOME%\\bin', enabled: true }];
    const { result } = renderHook(() => usePathValidation(paths));

    await waitFor(() =>
      expect(result.current.validationCache.get('%JAVA_HOME%\\bin')).toBe('valid'),
    );
    expect(expandEnvVars).toHaveBeenCalledWith('%JAVA_HOME%\\bin');
    expect(validatePath).toHaveBeenCalledWith('C:\\expanded\\JAVA_HOME\\bin');
    expect(result.current.expandedCache.get('%JAVA_HOME%\\bin')).toBe(
      'C:\\expanded\\JAVA_HOME\\bin',
    );
  });

  it('展开失败时标记为 unknown 并保留展开缓存', async () => {
    expandEnvVars.mockResolvedValueOnce('%MISSING%\\bin');
    const paths: PathEntry[] = [{ path: '%MISSING%\\bin', enabled: true }];
    const { result } = renderHook(() => usePathValidation(paths));

    await waitFor(() =>
      expect(result.current.validationCache.get('%MISSING%\\bin')).toBe('unknown'),
    );
    expect(validatePath).not.toHaveBeenCalled();
  });

  it('验证失败时标记为 unknown，不阻塞后续条目', async () => {
    validatePath.mockRejectedValueOnce(new Error('io error')).mockResolvedValue(true);
    const paths = entries(3);
    const { result } = renderHook(() => usePathValidation(paths));

    await waitFor(() => expect(result.current.validationCache.size).toBe(3));
    expect(result.current.validationCache.get('C:\\path0')).toBe('unknown');
    expect(result.current.validationCache.get('C:\\path1')).toBe('valid');
    expect(result.current.validationCache.get('C:\\path2')).toBe('valid');
  });

  it('并发数量不超过配置上限', async () => {
    let active = 0;
    let maxActive = 0;
    validatePath.mockImplementation(async () => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((resolve) => setTimeout(resolve, 0));
      active -= 1;
      return true;
    });

    const paths = entries(40);
    const { result } = renderHook(() => usePathValidation(paths));
    await waitFor(() => expect(result.current.validationCache.size).toBe(40));
    expect(maxActive).toBeLessThanOrEqual(VALIDATION_CONCURRENCY);
  });

  it('请求进行中 rerender 后会应用 in-flight 结果，不留下 pending', async () => {
    const resolvers = new Map<string, (value: boolean) => void>();
    validatePath.mockImplementation(
      (path: string) =>
        new Promise<boolean>((resolve) => {
          resolvers.set(path, resolve);
        }),
    );

    const initial = entries(2);
    const { result, rerender } = renderHook(({ paths }) => usePathValidation(paths), {
      initialProps: { paths: initial },
    });
    await waitFor(() => expect(resolvers.size).toBe(2));

    rerender({
      paths: [...initial, { path: 'C:\\path2', enabled: true }],
    });
    await waitFor(() => expect(resolvers.size).toBe(3));

    for (const resolve of resolvers.values()) resolve(true);
    await waitFor(() => {
      expect(result.current.validationCache.size).toBe(3);
      expect([...result.current.validationCache.values()].every((state) => state === 'valid')).toBe(
        true,
      );
    });
  });
  it('删除路径后清理旧缓存', async () => {
    const initial: PathEntry[] = [
      { path: 'C:\\one', enabled: true },
      { path: 'C:\\two', enabled: true },
    ];
    const { result, rerender } = renderHook(({ paths }) => usePathValidation(paths), {
      initialProps: { paths: initial },
    });
    await waitFor(() => expect(result.current.validationCache.size).toBe(2));

    rerender({ paths: [{ path: 'C:\\two', enabled: true }] });
    await waitFor(() => expect(result.current.validationCache.has('C:\\one')).toBe(false));
    expect(result.current.validationCache.get('C:\\two')).toBe('valid');
  });
});
