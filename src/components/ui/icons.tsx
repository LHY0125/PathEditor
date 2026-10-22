/**
 * 手写内联 SVG 图标集 —— 不引入图标库依赖。
 *
 * 模块状态：当前生产代码仅使用 `MoreIcon`（Menu 的 ⋯ 触发按钮）。
 * `PlusIcon` / `PencilIcon` / `TrashIcon` / `ArrowUpIcon` / `ArrowDownIcon` 是为工具栏
 * 规划的图标语言预留的 —— 工具栏改版最终选择了文字标签，点名因此未接线。
 * 本波**刻意保留**并由 `tests/unit/icons.test.tsx` 覆盖，待后续波次决定采用或移除；
 * 不要当作死代码顺手删除，也不要临时接线到工具栏。
 */

type IconProps = { className?: string };

const BASE = {
  viewBox: '0 0 16 16',
  width: 14,
  height: 14,
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  'aria-hidden': true,
};

export function PlusIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 3v10M3 8h10" />
    </svg>
  );
}

export function PencilIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M11.5 2.5l2 2-8 8H3.5v-2l8-8z" />
    </svg>
  );
}

export function TrashIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8h5.8l.6-8" />
    </svg>
  );
}

export function ArrowUpIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 13V3M4 7l4-4 4 4" />
    </svg>
  );
}

export function ArrowDownIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 3v10M4 9l4 4 4-4" />
    </svg>
  );
}

export function MoreIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <circle cx="3.5" cy="8" r="1" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
      <circle cx="12.5" cy="8" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}
