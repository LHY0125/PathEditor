/**
 * 快照预览。
 *
 * **掩码是默认**：`value === null && sensitive` 表示 Rust 侧已掩码，
 * 前端不做「拿到明文再隐藏」的动作。
 *
 * **显式 reveal**：敏感变量旁提供「显示」按钮，
 * 点击后调 `history_reveal_env_value` 取单个变量的明文。
 *
 * ⚠️ 明文**只存在本组件的局部 state**，不写入任何 store：
 * 写入 store 会让明文进入 DevTools / 持久状态，等于绕过了「显式动作」的语义。
 * 切换快照（`snapshot.uid` 变化）时自动清空。
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend, type SnapshotView } from '@/services/backend';

interface Props {
  snapshot: SnapshotView;
}

/** 变量值区：敏感值在 Rust 侧已掩码为 `null`，这里只做「不可用」的呈现。 */
function maskedOrValue(value: string | null, sensitive: boolean, emptyLabel: string): string {
  if (value !== null && value !== '') return value;
  return sensitive ? '••••••' : emptyLabel;
}

export function SnapshotDetail({ snapshot }: Props) {
  const { t } = useTranslation();
  const empty = t('history.emptyValue');

  // `hive:name` → 明文。键里带 uid 由下面的重置逻辑保证不跨快照泄漏。
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  // 正在请求中的键，用于禁用按钮与显示省略号
  const [pending, setPending] = useState<string | null>(null);
  // reveal 失败的键 → 错误文案（例如 Unsupported 类型返回 NotFound）
  const [errors, setErrors] = useState<Record<string, string>>({});
  // 记录明文所属的快照；uid 变化时丢弃所有已揭示的值
  const [revealedFor, setRevealedFor] = useState(snapshot.uid);

  if (revealedFor !== snapshot.uid) {
    // 渲染期同步重置：比 useEffect 少一帧明文残留
    setRevealedFor(snapshot.uid);
    setRevealed({});
    setErrors({});
    setPending(null);
  }

  const pathSection = (label: string, entries: SnapshotView['system']) => (
    <section className="mb-3">
      <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
        {label}（{entries.length}）
      </h4>
      {entries.length === 0 ? (
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {t('history.noEntries')}
        </p>
      ) : (
        <ol className="space-y-0.5">
          {entries.map((e, i) => (
            <li
              key={`${e.path}-${i}`}
              className="truncate text-xs"
              style={{
                color: e.enabled ? 'var(--app-fg)' : 'var(--app-disabled-fg)',
                textDecoration: e.enabled ? 'none' : 'line-through',
              }}
              title={e.path}
            >
              {e.path}
            </li>
          ))}
        </ol>
      )}
    </section>
  );

  /** 点击「显示」：取单个变量的明文，只放进局部 state。 */
  const handleReveal = (hive: string, name: string) => {
    const key = `${hive}:${name}`;
    setPending(key);
    void backend
      .historyRevealEnvValue(snapshot.uid, hive, name)
      .then((plain) => {
        setRevealed((prev) => ({ ...prev, [key]: plain }));
        setErrors((prev) => {
          const next = { ...prev };
          delete next[key];
          return next;
        });
      })
      .catch((err: unknown) => {
        const message =
          typeof err === 'object' && err !== null && 'message' in err
            ? String((err as { message: unknown }).message)
            : t('history.revealFailed');
        setErrors((prev) => ({ ...prev, [key]: message }));
      })
      .finally(() => setPending(null));
  };

  return (
    <div className="p-3">
      <div className="mb-3">
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {new Date(snapshot.createdAt).toLocaleString()} · {snapshot.kind}
        </p>
      </div>

      {pathSection(t('history.systemPath'), snapshot.system)}
      {pathSection(t('history.userPath'), snapshot.user)}

      <section>
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('history.envVars')}（{snapshot.env.length}）
        </h4>
        {snapshot.env.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('history.noEntries')}
          </p>
        ) : (
          <ul className="space-y-1">
            {snapshot.env.map((v) => {
              const key = `${v.hive}:${v.name}`;
              const plain = revealed[key];
              const err = errors[key];
              // 只有被掩码、且该类型可恢复的变量才提供「显示」按钮
              const canReveal = v.sensitive && v.value === null && v.restorable;
              return (
                <li key={key} className="text-xs">
                  <span className="flex items-center gap-1">
                    <span className="font-medium" style={{ color: 'var(--app-fg)' }}>
                      {v.name}
                    </span>
                    <span style={{ color: 'var(--app-fg-faint)' }}>{v.hive}</span>
                    {v.sensitive && (
                      <span
                        className="rounded px-1"
                        style={{ background: 'var(--app-warning)', color: 'var(--app-accent-fg)' }}
                        title={t('history.maskedTitle')}
                      >
                        {t('history.masked')}
                      </span>
                    )}
                    {canReveal && plain === undefined && (
                      <button
                        type="button"
                        className="rounded px-1"
                        style={{
                          color: 'var(--app-accent)',
                          background: 'transparent',
                          border: '1px solid var(--app-border)',
                        }}
                        disabled={pending === key}
                        onClick={() => handleReveal(v.hive, v.name)}
                      >
                        {pending === key ? t('history.revealing') : t('history.reveal')}
                      </button>
                    )}
                  </span>
                  <span
                    className="block truncate font-mono"
                    style={{ color: 'var(--app-fg-muted)' }}
                    title={plain ?? v.value ?? ''}
                  >
                    {plain ?? maskedOrValue(v.value, v.sensitive, empty)}
                  </span>
                  {err !== undefined && (
                    <span className="block text-xs" style={{ color: 'var(--app-danger)' }}>
                      {err}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
