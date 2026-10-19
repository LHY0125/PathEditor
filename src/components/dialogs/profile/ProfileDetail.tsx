import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';
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
        <div className="text-center py-10 text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
          {hasProfiles ? t('profile.selectProfile') : t('profile.noProfiles')}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 p-3 overflow-auto">
      <div className="flex items-center gap-2 mb-3">
        <span className="font-semibold text-xs">{data.name}</span>
        <span className="text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
          {data.modified}
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

      {renameOpen && (
        <div className="flex gap-2 mb-2">
          <input
            type="text"
            value={renameValue}
            onChange={(event) => onRenameValueChange(event.target.value)}
            className="field-input text-xs"
          />
          <Button size="sm" variant="primary" onClick={onRename}>
            {t('button.save')}
          </Button>
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
      <div className="text-xs font-medium mb-1" style={{ color: 'var(--app-fg-muted)' }}>
        {title}
      </div>
      {paths.length === 0 ? (
        <div className="text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
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
                color: entry.enabled ? 'var(--app-fg)' : 'var(--app-danger)',
                textDecoration: entry.enabled ? 'none' : 'line-through',
              }}
            >
              <span
                style={{
                  color: entry.enabled ? 'var(--app-success)' : 'var(--app-danger)',
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
