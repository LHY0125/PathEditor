import { useState } from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Modal } from '@/components/ui/Modal';
import { isAnyModalOpen, resetModalStateForTests } from '@/core/modal-state';

function Harness({ dismissible = true }: { dismissible?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        open
      </button>
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        ariaLabel="测试对话框"
        dismissible={dismissible}
      >
        <button type="button">first</button>
        <button type="button">last</button>
      </Modal>
    </>
  );
}

function openDialog() {
  render(<Harness />);
  const trigger = screen.getByRole('button', { name: 'open' });
  trigger.focus();
  fireEvent.click(trigger);
  return trigger;
}

describe('Modal 可访问性契约', () => {
  afterEach(() => {
    cleanup();
    resetModalStateForTests();
  });

  it('暴露 dialog 语义、aria-modal 与无障碍名称', () => {
    openDialog();
    const dialog = screen.getByRole('dialog', { name: '测试对话框' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  it('打开时焦点移入首个可聚焦元素，关闭后归还触发按钮', () => {
    const trigger = openDialog();

    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'first' }));

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('Tab / Shift+Tab 在弹窗内循环', () => {
    openDialog();
    const first = screen.getByRole('button', { name: 'first' });
    const last = screen.getByRole('button', { name: 'last' });

    last.focus();
    fireEvent.keyDown(last, { key: 'Tab' });
    expect(document.activeElement).toBe(first);

    first.focus();
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);
  });

  it('打开期间标记模态状态，关闭后释放', () => {
    expect(isAnyModalOpen()).toBe(false);
    openDialog();
    expect(isAnyModalOpen()).toBe(true);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(isAnyModalOpen()).toBe(false);
  });

  it('dismissible=false 时遮罩与 Escape 都不关闭', () => {
    render(<Harness dismissible={false} />);
    fireEvent.click(screen.getByRole('button', { name: 'open' }));
    const dialog = screen.getByRole('dialog', { name: '测试对话框' });

    fireEvent.click(dialog.parentElement as HTMLElement);
    expect(screen.queryByRole('dialog')).not.toBeNull();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeNull();
  });
});
