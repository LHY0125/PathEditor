import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useEnvStore } from '@/store/env-store';
import { displayValue, envVarKey, filterEnvVars } from '@/core/env-var';
import { Button } from '@/components/ui/Button';
import { RefGraphDialog } from '@/components/env/RefGraphDialog';
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

// 行内操作按钮为 32px（Button sm）；行高至少 34px 才能上下各留 1px，
// 否则相邻两行的按钮热区会重叠。
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
  const scrollToKey = useEnvStore((s) => s.scrollToKey);
  const parentRef = useRef<HTMLDivElement>(null);
  /** 打开引用图的变量名；`null` 表示对话框关闭。 */
  const [refsName, setRefsName] = useState<string | null>(null);

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

  // 搜索定位：滚动到目标变量后清空请求。
  useEffect(() => {
    if (scrollToKey === null) return;
    const index = rows.findIndex((meta) => envVarKey(meta) === scrollToKey);
    if (index < 0) {
      // 首次挂载时快照还没到（rows 为空）：保留请求，等 rows 更新后重试。
      return;
    }
    virtualizer.scrollToIndex(index, { align: 'center' });
    useEnvStore.getState().setScrollToKey(null);
  }, [scrollToKey, rows, virtualizer]);

  /** 行内小操作按钮：复用按钮原语的 ghost 变体，尺寸收窄以适配 28px 行高。 */

  return (
    <div className="flex flex-col h-full" data-testid="env-var-table">
      <div className="env-var-guide">
        <span>{t('envVar.pathGuide')}</span>
        {onGoToPath && (
          <button type="button" className="env-var-guide-link" onClick={onGoToPath}>
            {t('envVar.pathGuideAction')}
          </button>
        )}
      </div>
      {/* sticky 大写表头，与 PathTable 共用 .path-table-head 规范 */}
      <div className="path-table-head">
        <div className="w-8 px-2 py-1">#</div>
        <div className="px-2 py-1 flex-1">{t('envVar.name')}</div>
        <div className="px-2 py-1 flex-[2]">{t('envVar.value')}</div>
        <div className="w-24 px-2 py-1">{t('envVar.type')}</div>
        <div className="w-14 px-2 py-1">{t('envVar.source')}</div>
        <div className="w-52 px-2 py-1">{t('envVar.actions')}</div>
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto">
        {rows.length === 0 && (
          <p className="px-3 py-4 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {snapshot === null
              ? t('status.loading')
              : searchQuery
                ? t('envVar.noResults')
                : t('envVar.empty')}
          </p>
        )}
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
                // E2E 用该属性精确定位行（区分同名跨 hive 变量），不可移除。
                data-env-var-key={key}
                data-index={rowIdx}
                onClick={() => onSelect?.(meta)}
                role="row"
                aria-selected={isSelected}
                // roving tabindex：选中行可 Tab 进入；无选中时首行是入口
                tabIndex={isSelected || (selectedKey === null && rowIdx === 0) ? 0 : -1}
                onFocus={() => {
                  if (!isSelected) onSelect?.(meta);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onSelect?.(meta);
                    return;
                  }
                  const next =
                    e.key === 'ArrowDown'
                      ? rows[rowIdx + 1]
                      : e.key === 'ArrowUp'
                        ? rows[rowIdx - 1]
                        : null;
                  if (!next) return;
                  e.preventDefault();
                  onSelect?.(next);
                  // 选中态更新后把焦点移到目标行（虚拟列表里目标通常已渲染）
                  requestAnimationFrame(() => {
                    parentRef.current
                      ?.querySelector<HTMLElement>(
                        `[data-index="${rowIdx + (e.key === 'ArrowDown' ? 1 : -1)}"]`,
                      )
                      ?.focus();
                  });
                }}
                // 交替底色按逻辑索引，选中态由 .is-selected 覆盖
                data-odd={rowIdx % 2 === 1}
                className={`path-row path-table-row${isSelected ? ' is-selected' : ''}`}
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <div className="w-8 env-var-cell env-var-cell-dim">{rowIdx + 1}</div>
                <div className="env-var-cell env-var-cell-mono flex-1" title={meta.name}>
                  {meta.name}
                </div>
                <div
                  className="env-var-cell env-var-cell-mono flex-[2]"
                  title={displayValue(meta, revealedValue, t('envVar.unsupportedValue'))}
                >
                  {displayValue(meta, revealedValue, t('envVar.unsupportedValue'))}
                </div>
                <div className="w-24 env-var-cell env-var-cell-dim">
                  {t(TYPE_LABEL_KEY[meta.kind])}
                </div>
                <div className="w-14 env-var-cell env-var-cell-dim">
                  {meta.hive === 'system' ? t('merge.system') : t('merge.user')}
                </div>
                <div className="w-52 px-2 flex items-center gap-1">
                  {meta.sensitive && (
                    <Button
                      size="sm"
                      variant="ghost"
                      title={editHint(meta, revealedValue === null, t)}
                      onClick={(e) => {
                        e.stopPropagation();
                        void (revealedValue === null ? reveal(meta) : hide(meta));
                      }}
                    >
                      {revealedValue === null ? t('envVar.show') : t('envVar.hide')}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`${t('button.refs')} ${meta.name}`}
                    title={t('refs.title', { name: meta.name })}
                    onClick={(e) => {
                      e.stopPropagation();
                      setRefsName(meta.name);
                    }}
                  >
                    {t('button.refs')}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={!meta.canEdit || (meta.sensitive && revealedValue === null)}
                    title={editHint(meta, revealedValue === null, t)}
                    aria-label={`${t('button.edit')} ${meta.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onEdit?.(meta);
                    }}
                  >
                    {t('button.edit')}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={!meta.canDelete}
                    title={editHint(meta, revealedValue === null, t)}
                    aria-label={`${t('button.delete')} ${meta.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete?.(meta);
                    }}
                  >
                    {t('button.delete')}
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
      {refsName !== null && <RefGraphDialog name={refsName} onClose={() => setRefsName(null)} />}
    </div>
  );
}

/**
 * 禁用原因提示。权限判定单一实现在 Rust（canEdit 已折算 hive 写能力），
 * 前端只做文案映射 —— 不区分「保护变量」与「无写权限」，统一 protectedHint
 * 。
 */
function editHint(meta: EnvVarMeta, masked: boolean, t: (key: string) => string): string {
  if (meta.kind === 'unsupported') return t('envVar.typeUnsupported');
  if (!meta.canEdit) return t('envVar.protectedHint');
  if (masked && meta.sensitive) return t('envVar.revealFirstHint');
  return '';
}
