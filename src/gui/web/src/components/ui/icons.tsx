/**
 * 手写内联 SVG 图标集 —— 不引入图标库依赖。
 *
 * 工具栏统一使用「图标 + 文字」：图标只做辅助识别，全部 `aria-hidden`；
 * 可访问名称仍由按钮文字提供，避免读屏重复朗读。
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

export function SearchIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <circle cx="7" cy="7" r="4" />
      <path d="M10 10l3.5 3.5" />
    </svg>
  );
}

export function ImportIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 2.5v7M5 7l3 3 3-3M3 12.5h10" />
    </svg>
  );
}

export function ExportIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 9.5v-7M5 5l3-3 3 3M3 12.5h10" />
    </svg>
  );
}

export function SaveIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M3 2.5h8l2 2v9H3z" />
      <path d="M5.5 2.5v3.5h5V2.5M5.5 13v-4h5v4" />
    </svg>
  );
}

export function UndoIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M5.5 5.5H3v-2.5M3 5.5c1.2-2 3.2-3 5.2-3 3 0 5.3 2.2 5.3 5.2S11.2 13 8.2 13c-1.5 0-2.8-.6-3.8-1.6" />
    </svg>
  );
}

export function RedoIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M10.5 5.5H13v-2.5M13 5.5c-1.2-2-3.2-3-5.2-3C4.8 2.5 2.5 4.7 2.5 7.7S4.8 12.9 7.8 12.9c1.5 0 2.8-.6 3.8-1.6" />
    </svg>
  );
}

export function RefreshIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.5V6H9.5" />
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
