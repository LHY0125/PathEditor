import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchStore } from '@/store/search-store';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import { envVarKey, type EnvVarMeta } from '@/core/env-var';
import { Button } from '@/components/ui/Button';
import type { SearchHit } from '@/services/backend';

/**
 * 全文检索面板。
 *
 * **高亮由 Rust 侧产出**：`SearchHit.snippet` 是 FTS5 的 `snippet()` 结果，
 * 命中处已用 `[]` 包裹。前端只负责把 `[...]` 渲染成 `<mark>`，
 * 不重新做一次匹配——两处匹配规则必然分叉。
 *
 * **空查询不发请求**（两层各守一次：core 返回空、前端不发起）。
 *
 * **结果必须可操作**：命中项提供「复制完整内容」；若该条在**当前** PATH /
 * 变量快照中仍存在，再提供「定位」按钮（索引是跨快照的，历史条目可能已不存在）。
 */
export function SearchPanel() {
  const { t } = useTranslation();
  const { query, submitted, hits, loading, error, setQuery, run } = useSearchStore();
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const envSnapshot = useEnvStore((s) => s.snapshot);
  const [copied, setCopied] = useState<string | null>(null);
  const [kindFilter, setKindFilter] = useState('all');
  const [fromDate, setFromDate] = useState('');
  const [toDate, setToDate] = useState('');
  const dateRangeInvalid = fromDate !== '' && toDate !== '' && fromDate > toDate;
  const [saved, setSaved] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem('savedSearches');
      const parsed: unknown = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : [];
    } catch {
      return [];
    }
  });

  // 首次进入时清空上一次会话的残留结果
  useEffect(() => {
    useSearchStore.getState().reset();
  }, []);

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    void run();
    const text = query.trim();
    if (!text) return;
    setSaved((prev) => {
      const next = [text, ...prev.filter((item) => item !== text)].slice(0, 8);
      try {
        localStorage.setItem('savedSearches', JSON.stringify(next));
      } catch {
        // localStorage 不可用时只影响记忆
      }
      return next;
    });
  };

  const removeSaved = (text: string) => {
    setSaved((prev) => {
      const next = prev.filter((item) => item !== text);
      try {
        localStorage.setItem('savedSearches', JSON.stringify(next));
      } catch {
        // ignore
      }
      return next;
    });
  };

  /** 把 FTS5 的 `[命中]` 片段渲染成高亮，其余部分原样转义。 */
  const renderSnippet = (snippet: string) => {
    const parts: React.ReactNode[] = [];
    let rest = snippet;
    let key = 0;
    while (rest.length > 0) {
      const open = rest.indexOf('[');
      const close = open === -1 ? -1 : rest.indexOf(']', open + 1);
      if (open === -1 || close === -1) {
        parts.push(rest);
        break;
      }
      if (open > 0) parts.push(rest.slice(0, open));
      parts.push(
        <mark
          key={key++}
          style={{ background: 'var(--app-warning)', color: 'var(--app-warning-fg)' }}
        >
          {rest.slice(open + 1, close)}
        </mark>,
      );
      rest = rest.slice(close + 1);
    }
    return parts;
  };

  const kindLabel = (kind: string) => {
    if (kind === 'path') return t('search.kindPath');
    if (kind === 'envName') return t('search.kindEnvName');
    if (kind === 'profile') return t('search.kindProfile');
    if (kind === 'snapshotNote') return t('search.kindSnapshotNote');
    return kind;
  };

  /** 在当前 PATH 快照里找完全匹配的条目（大小写不敏感）。 */
  const findPath = (content: string): { tab: 'system' | 'user'; index: number } | null => {
    const lower = content.toLowerCase();
    const systemIndex = sysPaths.findIndex((p) => p.path.toLowerCase() === lower);
    if (systemIndex >= 0) return { tab: 'system', index: systemIndex };
    const userIndex = userPaths.findIndex((p) => p.path.toLowerCase() === lower);
    if (userIndex >= 0) return { tab: 'user', index: userIndex };
    return null;
  };

  /** 在当前变量快照里找同名变量；索引是跨快照的，历史条目可能已不存在。 */
  const findEnv = (content: string): EnvVarMeta | null => {
    if (!envSnapshot) return null;
    const lower = content.toLowerCase();
    return (
      [...envSnapshot.system, ...envSnapshot.user].find((m) => m.name.toLowerCase() === lower) ??
      null
    );
  };

  const goPath = (loc: { tab: 'system' | 'user'; index: number }) => {
    const state = useAppStore.getState();
    state.setActiveTab(loc.tab);
    state.setSelectedIndices([loc.index]);
    state.setScrollToIndex(loc.index);
  };

  const goEnv = (meta: EnvVarMeta) => {
    const env = useEnvStore.getState();
    const key = envVarKey(meta);
    env.setHiveFilter('all');
    env.setSelectedKey(key);
    env.setScrollToKey(key);
    const state = useAppStore.getState();
    state.setActiveTab('allVars');
    state.setSelectedIndices([]);
  };

  // refUid 对同快照内的多条 PATH 是相同的，不能单独当 React key / 复制态键。
  const hitKey = (hit: SearchHit, index: number) =>
    `${hit.kind}|${hit.refUid}|${hit.content}|${index}`;

  const copy = async (hit: SearchHit, key: string) => {
    try {
      await navigator.clipboard.writeText(hit.content);
      setCopied(key);
      window.setTimeout(() => setCopied((cur) => (cur === key ? null : cur)), 1500);
    } catch {
      // 剪贴板不可用（权限 / 非安全上下文）：保持按钮原文案，不误报成功
    }
  };

  return (
    <div className="flex-1 min-h-0 overflow-y-auto p-3">
      <form onSubmit={onSubmit} className="search-form">
        <input
          className="field-input search-input"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('search.placeholder')}
          aria-label={t('search.placeholder')}
        />
        <Button type="submit" variant="secondary" disabled={loading}>
          {loading ? t('search.searching') : t('search.submit')}
        </Button>
      </form>

      <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
        <select
          className="field-input"
          value={kindFilter}
          onChange={(e) => setKindFilter(e.target.value)}
          aria-label={t('search.filterKind')}
        >
          <option value="all">{t('search.allKinds')}</option>
          <option value="path">{t('search.kindPath')}</option>
          <option value="envName">{t('search.kindEnvName')}</option>
          <option value="profile">{t('search.kindProfile')}</option>
          <option value="snapshotNote">{t('search.kindSnapshotNote')}</option>
        </select>
        <label className="flex items-center gap-1" style={{ color: 'var(--app-fg-muted)' }}>
          {t('search.from')}
          <input
            type="date"
            className="field-input"
            value={fromDate}
            onChange={(e) => setFromDate(e.target.value)}
          />
        </label>
        <label className="flex items-center gap-1" style={{ color: 'var(--app-fg-muted)' }}>
          {t('search.to')}
          <input
            type="date"
            className="field-input"
            value={toDate}
            onChange={(e) => setToDate(e.target.value)}
          />
        </label>
        {dateRangeInvalid && (
          <span role="alert" style={{ color: 'var(--app-danger)' }}>
            {t('search.dateRangeInvalid')}
          </span>
        )}
        {saved.length > 0 && (
          <>
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('search.savedSearches')}</span>
            {saved.map((text) => (
              <span key={text} className="tab-pill inline-flex items-center gap-1">
                <button type="button" onClick={() => setQuery(text)}>
                  {text}
                </button>
                <button
                  type="button"
                  aria-label={t('search.removeSaved', { query: text })}
                  onClick={() => removeSaved(text)}
                >
                  ×
                </button>
              </span>
            ))}
          </>
        )}
      </div>

      {error !== null && (
        <p role="alert" className="text-xs" style={{ color: 'var(--app-danger)' }}>
          {error}
        </p>
      )}

      {error === null && submitted !== '' && hits.length === 0 && !loading && (
        <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('search.noResults')}
        </p>
      )}

      {hits.length > 0 && (
        <>
          <p className="search-count">{t('search.resultCount', { count: hits.length })}</p>
          <ul className="space-y-1">
            {hits
              .filter((hit) => {
                if (kindFilter !== 'all' && hit.kind !== kindFilter) return false;
                const from = fromDate ? new Date(`${fromDate}T00:00:00`).getTime() : null;
                const to = toDate ? new Date(`${toDate}T23:59:59.999`).getTime() : null;
                if (from !== null && hit.createdAt < from) return false;
                if (to !== null && hit.createdAt > to) return false;
                return true;
              })
              .map((hit, index) => {
                const key = hitKey(hit, index);
                const pathLoc = hit.kind === 'path' ? findPath(hit.content) : null;
                const envMeta = hit.kind === 'envName' ? findEnv(hit.content) : null;
                return (
                  <li key={key} className="text-xs search-hit">
                    <div className="search-hit-main">
                      <span className="search-kind rounded px-1">{kindLabel(hit.kind)}</span>
                      <span className="search-snippet font-mono">{renderSnippet(hit.snippet)}</span>
                    </div>
                    <div className="search-hit-actions">
                      {hit.kind === 'path' && (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={pathLoc === null}
                          title={pathLoc === null ? t('search.openFailed') : undefined}
                          onClick={() => {
                            if (pathLoc) goPath(pathLoc);
                          }}
                        >
                          {t('search.openPath')}
                        </Button>
                      )}
                      {hit.kind === 'envName' && (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={envMeta === null}
                          title={envMeta === null ? t('search.openFailed') : undefined}
                          onClick={() => {
                            if (envMeta) goEnv(envMeta);
                          }}
                        >
                          {t('search.openVar')}
                        </Button>
                      )}
                      <Button size="sm" variant="ghost" onClick={() => void copy(hit, key)}>
                        {copied === key ? t('search.copied') : t('search.copy')}
                      </Button>
                    </div>
                  </li>
                );
              })}
          </ul>
        </>
      )}
    </div>
  );
}
