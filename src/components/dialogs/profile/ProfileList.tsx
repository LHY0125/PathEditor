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
        <div className="text-xs text-center py-6" style={{ opacity: 0.5 }}>
          {t('profile.noProfiles')}
        </div>
      ) : (
        profiles.map((profile) => (
          <div
            key={profile.name}
            onClick={() => onSelect(profile.name)}
            className="px-2 py-1.5 text-sm rounded cursor-pointer mb-0.5"
            style={{
              backgroundColor: selected === profile.name ? 'rgba(59,130,246,0.15)' : 'transparent',
              color: selected === profile.name ? '#3b82f6' : 'var(--app-fg)',
            }}
          >
            {profile.name}
          </div>
        ))
      )}
    </div>
  );
}
