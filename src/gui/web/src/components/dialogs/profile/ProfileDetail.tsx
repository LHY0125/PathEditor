import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';
import type { PathEntry } from '@/core/path-entry';
import { useState } from 'react';
import type { ProfileDiff, ProfilePathsView } from '@/services/backend';

interface ProfileDetailProps {
  data: ProfilePathsView | null;
  hasProfiles: boolean;
  renameOpen: boolean;
  renameValue: string;
  onRenameOpen: () => void;
  onRenameValueChange: (value: string) => void;
  onApply: () => void;
  onRename: () => void;
  onDelete: (name: string) => void;
  others: string[];
  diff: ProfileDiff | null;
  diffLoading: boolean;
  onCompare: (other: string) => void;
  onClearDiff: () => void;
}

export function ProfileDetail({
  data,
  hasProfiles,
  renameOpen,
  renameValue,
  onRenameOpen,
  onRenameValueChange,
  onApply,
  onRename,
  onDelete,
  others,
  diff,
  diffLoading,
  onCompare,
  onClearDiff,
}: ProfileDetailProps) {
  const { t } = useTranslation();
  const [compareWith, setCompareWith] = useState('');

  if (!data) {
    return (
      <div className="flex-1 p-3 overflow-auto">
        <div className="text-center py-10 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {hasProfiles ? t('profile.selectProfile') : t('profile.noProfiles')}
        </div>
      </div>
    );
  }

  if (diff) return <ProfileDiffView diff={diff} onClose={onClearDiff} />;

  return (
    <div className="flex-1 p-3 overflow-auto">
      <div className="flex items-center gap-2 mb-3">
        <span className="font-semibold text-xs">{data.name}</span>
        <span className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {new Date(data.updatedAt).toLocaleString()}
        </span>
      </div>

      <div className="flex gap-1.5 mb-3">
        <Button size="sm" variant="primary" onClick={onApply}>
          {t('profile.apply')}
        </Button>
        <Button size="sm" onClick={onRenameOpen}>
          {t('profile.rename')}
        </Button>
        <Button size="sm" variant="danger" onClick={() => onDelete(data.name)}>
          {t('profile.delete')}
        </Button>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
        <select
          className="field-input"
          value={compareWith}
          onChange={(event) => setCompareWith(event.target.value)}
          aria-label={t('profile.compareWith')}
        >
          <option value="">{t('profile.comparePick')}</option>
          {others.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        <Button
          size="sm"
          disabled={!compareWith || diffLoading}
          onClick={() => onCompare(compareWith)}
        >
          {diffLoading ? t('profile.comparing') : t('profile.compare')}
        </Button>
      </div>

      {renameOpen && (
        <div className="flex gap-2 mb-2">
          <input
            type="text"
            value={renameValue}
            onChange={(event) => onRenameValueChange(event.target.value)}
            aria-label={t('profile.rename')}
            className="field-input"
          />
          <Button size="sm" variant="primary" onClick={onRename}>
            {t('button.save')}
          </Button>
        </div>
      )}

      <PathSection
        title={`${t('merge.system')} PATH (${data.system.length})`}
        paths={data.system}
      />
      <PathSection title={`${t('merge.user')} PATH (${data.user.length})`} paths={data.user} />
    </div>
  );
}

function PathSection({ title, paths }: { title: string; paths: PathEntry[] }) {
  const { t } = useTranslation();
  return (
    <div className="mb-2">
      <div className="text-xs font-medium mb-1" style={{ color: 'var(--app-fg-muted)' }}>
        {title}
      </div>
      {paths.length === 0 ? (
        <div className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
          {t('profile.empty')}
        </div>
      ) : (
        <div className="space-y-0.5 max-h-48 overflow-auto">
          {paths.map((entry) => (
            <div
              key={entry.path}
              className="text-xs font-mono px-2 py-0.5 rounded flex items-center gap-1.5"
              style={{
                backgroundColor: 'var(--app-surface)',
                color: entry.enabled ? 'var(--app-fg)' : 'var(--app-disabled-fg)',
                textDecoration: entry.enabled ? 'none' : 'line-through',
              }}
            >
              <span
                style={{
                  color: entry.enabled ? 'var(--app-success)' : 'var(--app-disabled-fg)',
                  fontSize: 10,
                }}
              >
                {entry.enabled ? '●' : '○'}
              </span>
              {entry.path}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** 两个配置的差异摘要（不重新拉快照，直接用 core 的 ProfileDiff）。 */
function ProfileDiffView({ diff, onClose }: { diff: ProfileDiff; onClose: () => void }) {
  const { t } = useTranslation();
  const pathSide = (label: string, side: ProfileDiff['paths']['system']) => (
    <div className="mb-2 text-xs">
      <div className="font-medium" style={{ color: 'var(--app-fg-muted)' }}>
        {label}：+{side.added.length} / -{side.removed.length} / ↔{side.reordered.length} / ◐
        {side.toggled.length}
      </div>
      {[
        ...side.added.map((entry) => `+ ${entry.path}`),
        ...side.removed.map((entry) => `- ${entry.path}`),
      ].map((line) => (
        <div key={line} className="font-mono truncate" title={line}>
          {line}
        </div>
      ))}
    </div>
  );
  return (
    <div className="flex-1 p-3 overflow-auto">
      <div className="mb-3 flex items-center gap-2">
        <span className="text-xs font-semibold">
          {diff.left} ↔ {diff.right}
        </span>
        <Button size="sm" variant="ghost" onClick={onClose}>
          {t('profile.closeCompare')}
        </Button>
      </div>
      {(diff.tagsAdded.length > 0 || diff.tagsRemoved.length > 0) && (
        <p className="mb-2 text-xs">
          {t('profile.tagsDiff', {
            added: diff.tagsAdded.join(', ') || '—',
            removed: diff.tagsRemoved.join(', ') || '—',
          })}
        </p>
      )}
      {pathSide(`${t('merge.system')} PATH`, diff.paths.system)}
      {pathSide(`${t('merge.user')} PATH`, diff.paths.user)}
      <div className="mb-2 text-xs">
        {t('profile.envDiff', {
          added: diff.paths.envAdded.join(', ') || '—',
          removed: diff.paths.envRemoved.join(', ') || '—',
          modified: diff.paths.envModified.join(', ') || '—',
        })}
      </div>
    </div>
  );
}
