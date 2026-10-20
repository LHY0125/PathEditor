import { useTranslation } from 'react-i18next';
import type { SnapshotView } from '@/services/backend';

interface Props {
  snapshot: SnapshotView;
}

/** 变量值区：敏感值在 Rust 侧已掩码为 `null`，这里只做「不可用」的呈现。 */
function maskedOrValue(value: string | null, sensitive: boolean): string {
  if (value !== null && value !== '') return value;
  return sensitive ? '••••••' : '（空）';
}

/**
 * 快照预览。
 *
 * **不在前端解密或补全明文**：`value === null && sensitive` 表示 Rust 侧已掩码，
 * 这里只显示圆点。需要明文必须走独立的 reveal 命令（M2 接入 UI），
 * 当前面板刻意不提供该入口 —— 历史面板的默认视图恒为掩码（spec D5）。
 */
export function SnapshotDetail({ snapshot }: Props) {
  const { t } = useTranslation();

  const pathSection = (label: string, entries: SnapshotView['system']) => (
    <section className="mb-3">
      <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
        {label}（{entries.length}）
      </h4>
      {entries.length === 0 ? (
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {t('history.noEntries', '无')}
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

  return (
    <div className="p-3">
      <div className="mb-3">
        <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
          {new Date(snapshot.createdAt).toLocaleString()} · {snapshot.kind}
        </p>
      </div>

      {pathSection(t('history.systemPath', '系统 PATH'), snapshot.system)}
      {pathSection(t('history.userPath', '用户 PATH'), snapshot.user)}

      <section>
        <h4 className="mb-1 text-xs font-semibold" style={{ color: 'var(--app-fg-muted)' }}>
          {t('history.envVars', '环境变量')}（{snapshot.env.length}）
        </h4>
        {snapshot.env.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-faint)' }}>
            {t('history.noEntries', '无')}
          </p>
        ) : (
          <ul className="space-y-1">
            {snapshot.env.map((v) => (
              <li key={`${v.hive}-${v.name}`} className="text-xs">
                <span className="flex items-center gap-1">
                  <span className="font-medium" style={{ color: 'var(--app-fg)' }}>
                    {v.name}
                  </span>
                  <span style={{ color: 'var(--app-fg-faint)' }}>{v.hive}</span>
                  {v.sensitive && (
                    <span
                      className="rounded px-1"
                      style={{ background: 'var(--app-warning)', color: 'var(--app-accent-fg)' }}
                      title={t('history.maskedTitle', '敏感值默认掩码')}
                    >
                      {t('history.masked', '已掩码')}
                    </span>
                  )}
                </span>
                <span
                  className="block truncate font-mono"
                  style={{ color: 'var(--app-fg-muted)' }}
                  title={v.value ?? ''}
                >
                  {maskedOrValue(v.value, v.sensitive)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
