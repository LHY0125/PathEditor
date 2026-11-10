import { describe, it, expect, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, within, cleanup } from '@testing-library/react';
import i18n from '@/i18n';
import { StatusBar } from '@/components/layout/StatusBar';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

beforeEach(() => {
  useAppStore.setState({
    activeTab: 'system',
    sysPaths: [
      { path: 'C:\\Windows', enabled: true },
      { path: 'C:\\Tools', enabled: true },
    ],
    userPaths: [{ path: 'D:\\Bin', enabled: true }],
  });
  useEnvStore.setState({ snapshot: null });
});

afterEach(() => {
  cleanup();
  useAppStore.setState({ activeTab: 'system', sysPaths: [], userPaths: [] });
  useEnvStore.setState({ snapshot: null });
});

function meta(name: string, hive: EnvVarMeta['hive']): EnvVarMeta {
  return {
    name,
    kind: 'string',
    hive,
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: null,
    revision: 'rev-1',
  };
}

describe('StatusBar 计数', () => {
  it('PATH 页统计 PATH 条目，不受环境变量快照影响', () => {
    useEnvStore.setState({
      snapshot: {
        system: [meta('windir', 'system')],
        user: [],
        capturedAt: 0,
      },
    });
    render(<StatusBar />);
    const group = screen.getByRole('group', { name: 'PATH 条目统计' });
    expect(within(group).getByText('系统').parentElement).toHaveTextContent('系统 2');
    expect(within(group).getByText('用户').parentElement).toHaveTextContent('用户 1');
    expect(within(group).getByText('总计').parentElement).toHaveTextContent('总计 3');
  });

  it('全部变量页切换为环境变量计数', () => {
    useAppStore.setState({ activeTab: 'allVars' });
    useEnvStore.setState({
      snapshot: {
        system: [meta('windir', 'system'), meta('JAVA_HOME', 'system')],
        user: [meta('TEMP', 'user')],
        capturedAt: 0,
      },
    });
    render(<StatusBar />);
    const group = screen.getByRole('group', { name: '环境变量统计' });
    expect(within(group).getByText('系统').parentElement).toHaveTextContent('系统 2');
    expect(within(group).getByText('用户').parentElement).toHaveTextContent('用户 1');
    expect(within(group).getByText('总计').parentElement).toHaveTextContent('总计 3');
  });

  it('环境变量尚未加载时显示占位符而不是 0', () => {
    useAppStore.setState({ activeTab: 'allVars' });
    render(<StatusBar />);
    const group = screen.getByRole('group', { name: '环境变量统计' });
    expect(within(group).getByText('系统').parentElement).toHaveTextContent('系统 —');
    expect(within(group).getByText('总计').parentElement).toHaveTextContent('总计 —');
  });
});
