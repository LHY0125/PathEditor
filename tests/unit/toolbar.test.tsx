import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
// 显式初始化 i18n：应用入口才 import '@/i18n'，单独渲染 ToolBar 时 t() 会回退成键名
import i18n from '@/i18n';
import { ToolBar } from '@/components/toolbar/ToolBar';
import { useAppStore } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效（同 button.test.tsx / menu.test.tsx）。
// i18n 必须在 render 前同步切换为中文并等待就绪：应用的 LanguageDetector 在 jsdom 下
// 会检出 en-US 并渲染英文文案；本文件断言按中文口诀编写，故显式钉住 zh-CN。
beforeAll(async () => {
  await i18n.changeLanguage('zh-CN');
});

afterEach(() => {
  cleanup();
});

/** 构造全部 16 个回调；未单独断言的回调也为 vi.fn，便于校验不会被误触发。 */
function makeProps() {
  return {
    onNew: vi.fn(),
    onEdit: vi.fn(),
    onBrowse: vi.fn(),
    onDelete: vi.fn(),
    onMoveUp: vi.fn(),
    onMoveDown: vi.fn(),
    onClean: vi.fn(),
    onImport: vi.fn(),
    onExport: vi.fn(),
    onSave: vi.fn(),
    onCancel: vi.fn(),
    onHelp: vi.fn(),
    onLanguage: vi.fn(),
    onDarkMode: vi.fn(),
    onAnalyze: vi.fn(),
    onProfiles: vi.fn(),
  };
}

const CAPS_ALL = {
  canReadSystem: true,
  canWriteSystem: true,
  canReadUser: true,
  canWriteUser: true,
};

/** 可写的系统 PATH 场景 */
function setWritable() {
  useAppStore.setState({
    activeTab: 'system',
    isAdmin: true,
    isModified: false,
    pathCapabilities: CAPS_ALL,
  });
}

function openMenu() {
  fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
}

describe('ToolBar 单行结构', () => {
  it('主行展示高频按钮，低频动作不在主行', () => {
    setWritable();
    render(<ToolBar {...makeProps()} />);
    expect(screen.getByTestId('toolbar').className).toContain('toolbar');
    for (const name of ['新建', '编辑', '删除', '上移', '下移', '撤销', '重做', '导入', '导出']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
    // 浏览 / 一键清理 / 帮助 / 分析 / 配置 已收进 ⋯ 菜单
    expect(screen.queryByRole('button', { name: '浏览...' })).toBeNull();
    expect(screen.queryByRole('button', { name: '一键清理' })).toBeNull();
    expect(screen.queryByRole('button', { name: '帮助' })).toBeNull();
  });

  it('保存按钮使用 savePath 文案而非对话框的「确定」', () => {
    setWritable();
    render(<ToolBar {...makeProps()} />);
    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '确定' })).toBeNull();
    expect(screen.queryByRole('button', { name: '取消' })).toBeNull();
  });

  it('脏标记不改变保存按钮的可访问名称', () => {
    setWritable();
    useAppStore.setState({ isModified: true });
    render(<ToolBar {...makeProps()} />);
    const save = screen.getByRole('button', { name: '保存' });
    const dot = save.querySelector('.dirty-dot');
    expect(dot).not.toBeNull();
    expect(dot?.getAttribute('aria-hidden')).toBe('true');
  });

  it('无写权限时编辑组、导入与保存禁用', () => {
    useAppStore.setState({
      activeTab: 'system',
      isAdmin: true,
      pathCapabilities: { ...CAPS_ALL, canWriteSystem: false },
    });
    render(<ToolBar {...makeProps()} />);
    for (const name of ['新建', '编辑', '删除', '上移', '下移', '导入', '保存']) {
      expect(screen.getByRole('button', { name })).toBeDisabled();
    }
    // 导出不写注册表，恒可用
    expect(screen.getByRole('button', { name: '导出' })).toBeEnabled();
  });
});

describe('ToolBar ⋯ 菜单', () => {
  it('菜单收纳低频动作，点击后回调被触发且菜单关闭', () => {
    setWritable();
    const props = makeProps();
    render(<ToolBar {...props} />);
    openMenu();
    for (const name of ['浏览...', '清理无效路径', /冲突分析/, /配置方案/, '切换语言', /^帮助/]) {
      expect(screen.getByRole('menuitem', { name })).toBeInTheDocument();
    }
    fireEvent.click(screen.getByRole('menuitem', { name: /冲突分析/ }));
    expect(props.onAnalyze).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('浏览与清理随写权限禁用', () => {
    useAppStore.setState({
      activeTab: 'system',
      isAdmin: true,
      pathCapabilities: { ...CAPS_ALL, canWriteSystem: false },
    });
    render(<ToolBar {...makeProps()} />);
    openMenu();
    expect(screen.getByRole('menuitem', { name: '浏览...' })).toBeDisabled();
    expect(screen.getByRole('menuitem', { name: '清理无效路径' })).toBeDisabled();
    // 只读动作不受写权限影响
    expect(screen.getByRole('menuitem', { name: /冲突分析/ })).toBeEnabled();
  });

  it('浅色态下主题条目显示「深色模式」并调用 onDarkMode', () => {
    setWritable();
    useThemeStore.setState({ isDark: false });
    const props = makeProps();
    render(<ToolBar {...props} />);
    openMenu();
    expect(screen.queryByRole('menuitem', { name: '浅色模式' })).toBeNull();
    fireEvent.click(screen.getByRole('menuitem', { name: '深色模式' }));
    expect(props.onDarkMode).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('深色态下主题条目显示「浅色模式」', () => {
    setWritable();
    useThemeStore.setState({ isDark: true });
    render(<ToolBar {...makeProps()} />);
    openMenu();
    expect(screen.getByRole('menuitem', { name: '浅色模式' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: '深色模式' })).toBeNull();
  });

  it('底部「取消并关闭」调用 onCancel', () => {
    setWritable();
    const props = makeProps();
    render(<ToolBar {...props} />);
    openMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: '取消并关闭' }));
    expect(props.onCancel).toHaveBeenCalledTimes(1);
    expect(props.onHelp).not.toHaveBeenCalled();
  });
});
