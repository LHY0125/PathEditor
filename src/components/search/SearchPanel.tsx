import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchStore } from '@/store/search-store';

/**
 * 全文检索面板（Task 10）。
 *
 * **高亮由 Rust 侧产出**：`SearchHit.snippet` 是 FTS5 的 `snippet()` 结果，
 * 命中处已用 `[]` 包裹。前端只负责把 `[...]` 渲染成 `<mark>`，
 * 不重新做一次匹配——两处匹配规则必然分叉。
 *
 * **空查询不发请求**（两层各守一次：core 返回空、前端不发起）。
 */
export function SearchPanel() {
  const { t } = useTranslation();
  const { query, submitted, hits, loading, error, setQuery, run } = useSearchStore();

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
        <mark key={key++} style={{ background: 'var(--app-warning)', color: 'inherit' }}>
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
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {t('search.noResults')}
        </p>
      )}

      {hits.length > 0 && (
        <ul className="space-y-1">
          {hits.map((hit) => (
            <li key={hit.refUid} className="text-xs">
              <span
                className="rounded px-1"
                style={{ background: 'var(--app-surface-raised)', color: 'var(--app-fg-muted)' }}
              >
                {kindLabel(hit.kind)}
              </span>
              <span className="ml-2 font-mono" style={{ color: 'var(--app-fg)' }}>
                {renderSnippet(hit.snippet)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
