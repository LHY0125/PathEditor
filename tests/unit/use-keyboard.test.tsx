import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, fireEvent, renderHook } from '@testing-library/react';
import { useAppStore, EMPTY_CAPABILITIES } from '@/store/app-store';
import { useKeyboard } from '@/hooks/use-keyboard';

function actions() {
  return {
    onNew: vi.fn(),
    onSave: vi.fn(),
    onDelete: vi.fn(),
    onUndo: vi.fn(),
    onRedo: vi.fn(),
    onHelp: vi.fn(),
  };
}

describe('useKeyboard', () => {
  beforeEach(() => {
    useAppStore.setState({
      activeTab: 'system',
      isAdmin: true,
      pathCapabilities: {
        canReadSystem: true,
        canWriteSystem: true,
        canReadUser: true,
        canWriteUser: true,
      },
    });
  });

  afterEach(() => cleanup());

  it('可写系统 PATH 时触发快捷键', async () => {
    const handlers = actions();
    renderHook(() => useKeyboard(handlers));
    await act(async () => {});

    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'y', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'n', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'Delete' });

    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(handlers.onUndo).toHaveBeenCalledTimes(1);
    expect(handlers.onRedo).toHaveBeenCalledTimes(1);
    expect(handlers.onNew).toHaveBeenCalledTimes(1);
    expect(handlers.onDelete).toHaveBeenCalledTimes(1);
  });

  it('不可写系统 PATH 时编辑快捷键无效，但 F1 仍然可用', async () => {
    useAppStore.setState({
      activeTab: 'system',
      isAdmin: false,
      pathCapabilities: {
        ...EMPTY_CAPABILITIES,
        canReadSystem: true,
        canReadUser: true,
        canWriteUser: true,
      },
    });
    const handlers = actions();
    renderHook(() => useKeyboard(handlers));
    await act(async () => {});

    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'Delete' });
    fireEvent.keyDown(window, { key: 'F1' });

    expect(handlers.onSave).not.toHaveBeenCalled();
    expect(handlers.onUndo).not.toHaveBeenCalled();
    expect(handlers.onDelete).not.toHaveBeenCalled();
    expect(handlers.onHelp).toHaveBeenCalledTimes(1);
  });

  it('普通用户可在用户 tab 使用快捷键', async () => {
    useAppStore.setState({
      activeTab: 'user',
      isAdmin: false,
      pathCapabilities: {
        canReadSystem: true,
        canWriteSystem: false,
        canReadUser: true,
        canWriteUser: true,
      },
    });
    const handlers = actions();
    renderHook(() => useKeyboard(handlers));
    await act(async () => {});

    fireEvent.keyDown(window, { key: 'n', ctrlKey: true });
    expect(handlers.onNew).toHaveBeenCalledTimes(1);
  });

  it('merged 视图不触发写操作', async () => {
    useAppStore.setState({ activeTab: 'merged' });
    const handlers = actions();
    renderHook(() => useKeyboard(handlers));
    await act(async () => {});

    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'Delete' });
    expect(handlers.onSave).not.toHaveBeenCalled();
    expect(handlers.onDelete).not.toHaveBeenCalled();
  });

  it('输入框内不触发全局编辑快捷键，Escape 会失焦', async () => {
    const handlers = actions();
    renderHook(() => useKeyboard(handlers));
    await act(async () => {});
    const input = document.createElement('input');
    document.body.appendChild(input);
    const blur = vi.spyOn(input, 'blur');

    fireEvent.keyDown(input, { key: 's', ctrlKey: true });
    expect(handlers.onSave).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: 'Escape' });
    expect(blur).toHaveBeenCalledTimes(1);
    input.remove();
  });
});
