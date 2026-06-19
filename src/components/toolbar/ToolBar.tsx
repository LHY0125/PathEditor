import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
import { btnClass, btnStyle } from '@/components/ui/buttons';
import { SearchInput } from './SearchInput';
import { ActionButtons } from './ActionButtons';
import { UndoRedoButtons } from './UndoRedoButtons';

interface ToolBarProps {
  onNew: () => void;
  onEdit: () => void;
  onBrowse: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onClean: () => void;
  onImport: () => void;
  onExport: () => void;
  onSave: () => void;
  onCancel: () => void;
  onHelp: () => void;
  onLanguage: () => void;
  onDarkMode: () => void;
  onAnalyze: () => void;
  onProfiles: () => void;
}

export function ToolBar(props: ToolBarProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const isModified = useAppStore((s) => s.isModified);

  return (
    <div className="space-y-2 pb-2 border-b" style={{ borderColor: 'var(--app-border)' }}>
      {/* 第一行: 搜索 + 系统按钮 */}
      <div className="flex items-center gap-2 flex-wrap">
        <SearchInput />
        <div className="flex-1" />
        <UndoRedoButtons />
        <button className={btnClass} style={btnStyle} disabled={!isAdmin} onClick={props.onImport}>
          {t('button.import')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onExport}>
          {t('button.export')}
        </button>
        <button
          className={btnClass}
          style={{
            ...btnStyle,
            backgroundColor: isModified ? '#2563eb' : btnStyle.backgroundColor,
            color: isModified ? '#fff' : btnStyle.color,
          }}
          disabled={!isAdmin}
          onClick={props.onSave}
        >
          {t('button.save')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onCancel}>
          {t('button.cancel')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onHelp}>
          {t('button.help')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onLanguage}>
          {t('button.language')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onAnalyze}>
          {t('button.analyze')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onProfiles}>
          {t('button.profiles')}
        </button>
        <button className={btnClass} style={btnStyle} onClick={props.onDarkMode}>
          {t('button.darkMode')}
        </button>
      </div>

      {/* 第二行: CRUD 操作 */}
      <ActionButtons
        onNew={props.onNew}
        onEdit={props.onEdit}
        onBrowse={props.onBrowse}
        onDelete={props.onDelete}
        onMoveUp={props.onMoveUp}
        onMoveDown={props.onMoveDown}
        onClean={props.onClean}
      />
    </div>
  );
}
