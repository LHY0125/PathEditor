import type { CSSProperties } from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

/** 所有变体共享的基础类 */
const BASE =
  'inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg border ' +
  'font-medium transition-colors select-none ' +
  'disabled:opacity-40 disabled:cursor-not-allowed disabled:pointer-events-none';

const SIZE: Record<ButtonSize, string> = {
  sm: 'h-6 px-2 text-xs',
  md: 'h-7 px-2.5 text-xs',
};

/**
 * 返回按钮的 className。变体通过自定义类名（btn-*）表达，
 * 具体颜色在 globals.css 中按主题切换 —— 避免在 TS 里硬编码色值。
 */
export function buttonVariants(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'md',
): string {
  return `${BASE} ${SIZE[size]} btn-${variant}`;
}

/**
 * @deprecated 过渡导出：Task 5-8 完成后删除。
 * 使用 Button 组件或 buttonVariants 代替。
 */
export const btnClass =
  'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';

/**
 * @deprecated 过渡导出：Task 6-7 完成后删除。
 *
 * **H4：值必须保持不变。** 消费方是 R7 范围外的 `EnvBackupPanel.tsx:30/127`（对话框）
 * 与 `EnvVarToolbar.tsx`。把 `--app-bg` 改成 `--app-surface` 属于跨范围视觉变更，
 * 会让对话框内按钮底色在当前浅色主题下从 `#fafafa` 变成 `#ffffff`。
 * 新语义层里 `--app-bg` 与 `--app-surface` 名字都保留，故旧值仍可用。
 */
export const btnStyle: CSSProperties = {
  backgroundColor: 'var(--app-bg)',
  color: 'var(--app-fg)',
  borderColor: 'var(--app-border)',
};
