import { useTranslation } from 'react-i18next';
import type { ProfileMeta } from '@/services/backend';

interface ProfileListProps {
  profiles: ProfileMeta[];
  selected: string | null;
  onSelect: (name: string) => void;
}

export function ProfileList({ profiles, selected, onSelect }: ProfileListProps) {
  const { t } = useTranslation();

  return (
    <div className="w-48 border-r overflow-auto p-2" style={{ borderColor: 'var(--app-border)' }}>
      {profiles.length === 0 ? (
        <div className="text-xs text-center py-6" style={{ color: 'var(--app-fg-subtle)' }}>
          {t('profile.noProfiles')}
        </div>
      ) : (
        profiles.map((profile) => (
          <div
            key={profile.name}
            onClick={() => onSelect(profile.name)}
            data-selected={selected === profile.name}
            className="dialog-list-item px-2 py-1.5 text-xs mb-0.5"
          >
            {profile.name}
          </div>
        ))
      )}
    </div>
  );
}
