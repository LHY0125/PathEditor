import { useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useEnvStore } from '@/store/env-store';
import { displayValue, envVarKey, filterEnvVars } from '@/core/env-var';
import type { EnvVarMeta } from '@/core/env-var';

interface EnvVarTableProps {
  searchQuery?: string;
  onSelect?: (meta: EnvVarMeta) => void;
  onEdit?: (meta: EnvVarMeta) => void;
  onDelete?: (meta: EnvVarMeta) => void;
  onGoToPath?: () => void;
  selectedKey?: string | null;
}

const TYPE_LABEL_KEY: Record<EnvVarMeta['kind'], string> = {
  string: 'envVar.typeString',
  expandString: 'envVar.typeExpand',
  unsupported: 'envVar.typeUnsupported',
};

// 与 PathTable 一致：28px 固定行高（UI 规范对齐）
const ROW_HEIGHT = 28;

export function EnvVarTable({
  searchQuery = '',
  onSelect,
  onEdit,
  onDelete,
  onGoToPath,
  selectedKey,
}: EnvVarTableProps) {
  const { t } = useTranslation();
  const snapshot = useEnvStore((s) => s.snapshot);
  const hiveFilter = useEnvStore((s) => s.hiveFilter);
  const revealed = useEnvStore((s) => s.revealed);
  const reveal = useEnvStore((s) => s.reveal);
  const hide = useEnvStore((s) => s.hide);
  const parentRef = useRef<HTMLDivElement>(null);

  // Path 由 Rust 侧过滤；此处防御性再滤一次，避免契约被破坏时泄漏到 UI。
  // 大小写不敏感比较：Windows 注册表变量名不区分大小写。
  const rows = useMemo(() => {
    if (snapshot === null) return [];
    return filterEnvVars(snapshot, hiveFilter, searchQuery).filter(
      (meta) => meta.name.toLowerCase() !== 'path',
    );
  }, [snapshot, hiveFilter, searchQuery]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    initialRect: { width: 800, height: 600 },
  });

  /** 行内小操作按钮：与工具栏描边按钮同风格，尺寸收窄以适配行高。 */
  const rowBtn =
    'px-1.5 text-xs rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';

  return (
    <div className="flex flex-col h-full" data-testid="env-var-table">
      <div className="flex items-center gap-2 px-2 py-1 text-xs opacity-70">
        <span>{t('envVar.pathGuide')}</span>
        {onGoToPath && (
          <button className="underline" onClick={onGoToPath}>
            {t('envVar.pathGuideAction')}
          </button>
        )}
      </div>
      {/* sticky 大写表头，与 PathTable 同规范 */}
      <div
        className="sticky top-0 z-10 flex text-left text-xs uppercase"
        style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
      >
        <div className="w-8 px-2 py-1">#</div>
        <div className="px-2 py-1 flex-1">{t('envVar.name')}</div>
        <div className="px-2 py-1 flex-[2]">{t('envVar.value')}</div>
        <div className="w-24 px-2 py-1">{t('envVar.type')}</div>
        <div className="w-14 px-2 py-1">{t('envVar.source')}</div>
        <div className="w-44 px-2 py-1">{t('envVar.actions')}</div>
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto">
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: '100%',
            position: 'relative',
          }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const rowIdx = virtualRow.index;
            const meta = rows[rowIdx];
            const key = envVarKey(meta);
            const revealedValue = revealed.get(key) ?? null;
            const isSelected = selectedKey === key;
            return (
              <div
                key={key}
                // Task 8 的 E2E 用该属性精确定位行（区分同名跨 hive 变量），不可移除。
                data-env-var-key={key}
                data-index={rowIdx}
                onClick={() => onSelect?.(meta)}
                className="cursor-pointer select-none flex items-center absolute top-0 left-0 w-full"
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                  backgroundColor: isSelected
                    ? 'var(--app-select-row)'
                    : rowIdx % 2 === 0
                      ? 'var(--app-list-bg)'
                      : 'var(--app-list-alt)',
                }}
              >
                <div
                  className="w-8 px-2 py-0.5 text-xs opacity-50"
                  style={{ color: 'var(--app-fg)' }}
                >
                  {rowIdx + 1}
                </div>
                <div className="px-2 py-0.5 text-sm truncate flex-1" title={meta.name}>
                  {meta.name}
                </div>
                <div
                  className="px-2 py-0.5 text-sm truncate flex-[2]"
                  title={displayValue(meta, revealedValue, t('envVar.unsupportedValue'))}
                >
                  {displayValue(meta, revealedValue, t('envVar.unsupportedValue'))}
                </div>
                <div className="w-24 px-2 py-0.5 text-xs opacity-70">
                  {t(TYPE_LABEL_KEY[meta.kind])}
                </div>
                <div className="w-14 px-2 py-0.5 text-xs opacity-70">
                  {meta.hive === 'system' ? t('merge.system') : t('merge.user')}
                </div>
                <div className="w-44 px-2 py-0.5 flex items-center gap-1">
                  {meta.sensitive && (
                    <button
                      className={rowBtn}
                      style={{
                        backgroundColor: 'var(--app-bg)',
                        borderColor: 'var(--app-border)',
                        color: 'var(--app-fg)',
                      }}
                      title={editHint(meta, revealedValue === null, t)}
                      onClick={(e) => {
                        e.stopPropagation();
                        void (revealedValue === null ? reveal(meta) : hide(meta));
                      }}
                    >
                      {revealedValue === null ? t('envVar.show') : t('envVar.hide')}
                    </button>
                  )}
                  <button
                    className={rowBtn}
                    style={{
                      backgroundColor: 'var(--app-bg)',
                      borderColor: 'var(--app-border)',
                      color: 'var(--app-fg)',
                    }}
                    disabled={!meta.canEdit || (meta.sensitive && revealedValue === null)}
                    title={editHint(meta, revealedValue === null, t)}
                    aria-label={`${t('button.edit')} ${meta.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onEdit?.(meta);
                    }}
                  >
                    {t('button.edit')}
                  </button>
                  <button
                    className={rowBtn}
                    style={{
                      backgroundColor: 'var(--app-bg)',
                      borderColor: 'var(--app-border)',
                      color: 'var(--app-fg)',
                    }}
                    disabled={!meta.canDelete}
                    title={editHint(meta, revealedValue === null, t)}
                    aria-label={`${t('button.delete')} ${meta.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete?.(meta);
                    }}
                  >
                    {t('button.delete')}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/**
 * 禁用原因提示。权限判定单一实现在 Rust（canEdit 已折算 hive 写能力），
 * 前端只做文案映射 —— 不区分「保护变量」与「无写权限」，统一 protectedHint
 * （计划 Task 7 的明确取舍，c1）。
 */
function editHint(meta: EnvVarMeta, masked: boolean, t: (key: string) => string): string {
  if (meta.kind === 'unsupported') return t('envVar.typeUnsupported');
  if (!meta.canEdit) return t('envVar.protectedHint');
  if (masked && meta.sensitive) return t('envVar.revealFirstHint');
  return '';
}
