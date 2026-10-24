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

  // 首次进入时清空上一次会话的残留结果
  useEffect(() => {
    useSearchStore.getState().reset();
  }, []);

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    void run();
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
    <div className="p-3">
      <form onSubmit={onSubmit} className="mb-3 flex gap-2">
        <input
          className="field-input"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('search.placeholder')}
          aria-label={t('search.placeholder')}
        />
        <button type="submit" className="tab-pill" disabled={loading}>
          {loading ? t('search.searching') : t('search.submit')}
        </button>
      </form>

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
        <ul className="space-y-1">
          {hits.map((hit, index) => {
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
      )}
    </div>
  );
}
