/**
 * 模态框打开计数（零 React 依赖）。
 *
 * 全局快捷键（Delete / Ctrl+S / Ctrl+N …）必须在任何模态框打开时暂停：
 * 否则用户在「帮助」「配置」里按 Delete，删掉的是主界面选中的路径。
 *
 * 用**计数**而不是布尔：对话框可以叠加（例如历史面板里再打开对比窗口），
 * 内层关闭时不能把外层的「打开」状态一起清掉。
 */

let openCount = 0;

/** 标记一个模态框已打开；返回的清理函数在关闭/卸载时调用（幂等）。 */
export function markModalOpen(): () => void {
  openCount += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    openCount -= 1;
  };
}

/** 当前是否有任何模态框处于打开状态。 */
export function isAnyModalOpen(): boolean {
  return openCount > 0;
}

/** 仅用于测试：重置计数，避免用例之间互相污染。 */
export function resetModalStateForTests(): void {
  openCount = 0;
}
