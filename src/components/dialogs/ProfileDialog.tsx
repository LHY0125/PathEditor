import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { ProfileDetail } from './profile/ProfileDetail';
import { ProfileList } from './profile/ProfileList';
import { useProfiles } from './profile/use-profiles';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ProfileDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const profiles = useProfiles(open, onClose);

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxWidth: '100%', maxHeight: '75vh' }}>
        <div
          className="flex items-center justify-between px-1 py-2"
          style={{ borderBottom: '1px solid var(--app-border)' }}
        >
          <h2 className="text-base font-semibold">{t('profile.title')}</h2>
          <div className="flex gap-2 items-center">
            <input
              type="text"
              value={profiles.newName}
              onChange={(event) => profiles.setNewName(event.target.value)}
              placeholder={t('profile.namePlaceholder')}
              className="field-input w-44"
            />
            <Button
              variant="primary"
              disabled={profiles.saving || !profiles.newName.trim()}
              onClick={profiles.handleSave}
            >
              {t('profile.save')}
            </Button>
            <Button
              variant="ghost"
              onClick={onClose}
              title={t('button.close')}
              aria-label={t('button.close')}
            >
              ✕
            </Button>
          </div>
        </div>

        <div className="flex flex-1 overflow-hidden">
          <ProfileList
            profiles={profiles.profiles}
            selected={profiles.selected}
            onSelect={profiles.handleLoad}
          />
          <ProfileDetail
            data={profiles.selectedData}
            hasProfiles={profiles.profiles.length > 0}
            renameOpen={profiles.renameOpen}
            renameValue={profiles.renameValue}
            onRenameOpen={() => {
              if (!profiles.selectedData) return;
              profiles.setRenameOpen(true);
              profiles.setRenameValue(profiles.selectedData.name);
            }}
            onRenameValueChange={profiles.setRenameValue}
            onApply={profiles.handleApply}
            onRename={profiles.handleRename}
            onDelete={profiles.handleDelete}
          />
        </div>
      </div>
    </Modal>
  );
}
