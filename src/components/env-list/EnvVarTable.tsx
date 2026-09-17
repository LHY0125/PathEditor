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

const ROW_HEIGHT = 34;

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
    overscan: 12,
  });

  return (
    <div className="flex flex-col h-full" data-testid="env-var-table">
      <div className="flex items-center gap-2 px-3 py-1.5 text-xs opacity-70">
        <span>{t('envVar.pathGuide')}</span>
        {onGoToPath && (
          <button className="underline" onClick={onGoToPath}>
            {t('envVar.pathGuideAction')}
          </button>
        )}
      </div>
      <div
        className="flex items-center gap-2 px-3 py-1 text-xs font-medium border-b"
        style={{ borderColor: 'var(--app-border)' }}
      >
        <span className="flex-1">{t('envVar.name')}</span>
        <span className="flex-[2]">{t('envVar.value')}</span>
        <span className="w-28">{t('envVar.type')}</span>
        <span className="w-16">{t('envVar.source')}</span>
        <span className="w-32">{t('envVar.actions')}</span>
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto">
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((item) => {
            const meta = rows[item.index];
            const key = envVarKey(meta);
            const revealedValue = revealed.get(key) ?? null;
            return (
              <div
                key={key}
                // Task 8 的 E2E 用该属性精确定位行（区分同名跨 hive 变量），不可移除。
                data-env-var-key={key}
                data-index={item.index}
                ref={virtualizer.measureElement}
                className={`flex items-center gap-2 px-3 text-sm border-b ${
                  selectedKey === key ? 'row-selected' : ''
                }`}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${item.start}px)`,
                  borderColor: 'var(--app-border)',
                  minHeight: ROW_HEIGHT,
                }}
                onClick={() => onSelect?.(meta)}
              >
                <span className="flex-1 truncate" title={meta.name}>
                  {meta.name}
                </span>
                <span className="flex-[2] truncate font-mono text-xs">
                  {displayValue(meta, revealedValue, t('envVar.unsupportedValue'))}
                </span>
                <span className="w-28 text-xs opacity-70">{t(TYPE_LABEL_KEY[meta.kind])}</span>
                <span className="w-16 text-xs opacity-70">
                  {meta.hive === 'system' ? t('merge.system') : t('merge.user')}
                </span>
                <span className="w-32 flex items-center gap-2">
                  {meta.sensitive && (
                    <button
                      className="text-xs underline"
                      onClick={(e) => {
                        e.stopPropagation();
                        void (revealedValue === null ? reveal(meta) : hide(meta));
                      }}
                    >
                      {revealedValue === null ? t('envVar.show') : t('envVar.hide')}
                    </button>
                  )}
                  <button
                    className="text-xs"
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
                    className="text-xs"
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
                </span>
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
