export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

/** 所有变体共享的基础类 */
const BASE =
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-[var(--radius-control)] border ' +
  'font-medium transition-colors select-none ' +
  'disabled:opacity-40 disabled:cursor-not-allowed disabled:pointer-events-none';

const SIZE: Record<ButtonSize, string> = {
  sm: 'h-8 px-3 text-xs',
  md: 'h-9 px-3.5 text-[13px]',
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
