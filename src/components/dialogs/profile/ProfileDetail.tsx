import { useTranslation } from 'react-i18next';
import type { PathEntry } from '@/core/path-entry';
import type { ProfileData } from '@/services/backend';

interface ProfileDetailProps {
  data: ProfileData | null;
  hasProfiles: boolean;
  renameOpen: boolean;
  renameValue: string;
  onRenameOpen: () => void;
  onRenameValueChange: (value: string) => void;
  onApply: () => void;
  onRename: () => void;
  onDelete: (name: string) => void;
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
}: ProfileDetailProps) {
  const { t } = useTranslation();

  if (!data) {
    return (
      <div className="flex-1 p-3 overflow-auto">
        <div className="text-center py-10 text-sm" style={{ opacity: 0.4 }}>
          {hasProfiles ? t('profile.selectProfile') : t('profile.noProfiles')}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 p-3 overflow-auto">
      <div className="flex items-center gap-2 mb-3">
        <span className="font-semibold text-sm">{data.name}</span>
        <span className="text-xs" style={{ opacity: 0.5 }}>
          {data.modified}
        </span>
      </div>

      <div className="flex gap-1.5 mb-3">
        <button
          className="px-3 py-1 text-xs rounded text-white"
          style={{ backgroundColor: '#3b82f6' }}
          onClick={onApply}
        >
          {t('profile.apply')}
        </button>
        <button
          className="px-3 py-1 text-xs rounded"
          style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)' }}
          onClick={onRenameOpen}
        >
          {t('profile.rename')}
        </button>
        <button
          className="px-3 py-1 text-xs rounded text-white"
          style={{ backgroundColor: '#ef4444' }}
          onClick={() => onDelete(data.name)}
        >
          {t('profile.delete')}
        </button>
      </div>

      {renameOpen && (
        <div className="flex gap-2 mb-2">
          <input
            type="text"
            value={renameValue}
            onChange={(event) => onRenameValueChange(event.target.value)}
            className="px-2 py-1 text-xs rounded border outline-none"
            style={{
              backgroundColor: 'var(--app-list-bg)',
              color: 'var(--app-fg)',
              borderColor: 'var(--app-border)',
            }}
          />
          <button
            className="px-2 py-1 text-xs rounded text-white"
            style={{ backgroundColor: '#3b82f6' }}
            onClick={onRename}
          >
            {t('button.save')}
          </button>
        </div>
      )}

      <PathSection title={`${t('merge.system')} PATH (${data.sys.length})`} paths={data.sys} />
      <PathSection title={`${t('merge.user')} PATH (${data.user.length})`} paths={data.user} />
    </div>
  );
}

function PathSection({ title, paths }: { title: string; paths: PathEntry[] }) {
  const { t } = useTranslation();
  return (
    <div className="mb-2">
      <div className="text-xs font-medium mb-1" style={{ opacity: 0.7 }}>
        {title}
      </div>
      {paths.length === 0 ? (
        <div className="text-xs" style={{ opacity: 0.4 }}>
          {t('profile.empty')}
        </div>
      ) : (
        <div className="space-y-0.5 max-h-48 overflow-auto">
          {paths.map((entry) => (
            <div
              key={entry.path}
              className="text-xs font-mono px-2 py-0.5 rounded flex items-center gap-1.5"
              style={{
                backgroundColor: 'var(--app-list-bg)',
                color: entry.enabled ? 'var(--app-fg)' : '#ef4444',
                textDecoration: entry.enabled ? 'none' : 'line-through',
                opacity: entry.enabled ? 1 : 0.5,
              }}
            >
              <span style={{ color: entry.enabled ? '#22c55e' : '#ef4444', fontSize: 10 }}>
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
