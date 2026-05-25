import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
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
}

export function ToolBar(props: ToolBarProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const isModified = useAppStore((s) => s.isModified);

  const sysBtnClass =
    'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';
  const sysBtnStyle = {
    backgroundColor: 'var(--app-bg)',
    color: 'var(--app-fg)',
    borderColor: 'var(--app-border)',
  };

  return (
    <div className="space-y-2 pb-2 border-b" style={{ borderColor: 'var(--app-border)' }}>
      {/* 第一行: 搜索 + 系统按钮 */}
      <div className="flex items-center gap-2 flex-wrap">
        <SearchInput />
        <div className="flex-1" />
        <UndoRedoButtons />
        <button
          className={sysBtnClass}
          style={sysBtnStyle}
          disabled={!isAdmin}
          onClick={props.onImport}
        >
          {t('button.import')}
        </button>
        <button className={sysBtnClass} style={sysBtnStyle} onClick={props.onExport}>
          {t('button.export')}
        </button>
        <button
          className={sysBtnClass}
          style={{
            ...sysBtnStyle,
            backgroundColor: isModified ? '#2563eb' : sysBtnStyle.backgroundColor,
            color: isModified ? '#fff' : sysBtnStyle.color,
          }}
          disabled={!isAdmin}
          onClick={props.onSave}
        >
          {t('button.save')}
        </button>
        <button className={sysBtnClass} style={sysBtnStyle} onClick={props.onCancel}>
          {t('button.cancel')}
        </button>
        <button className={sysBtnClass} style={sysBtnStyle} onClick={props.onHelp}>
          {t('button.help')}
        </button>
        <button className={sysBtnClass} style={sysBtnStyle} onClick={props.onLanguage}>
          {t('button.language')}
        </button>
        <button className={sysBtnClass} style={sysBtnStyle} onClick={props.onDarkMode}>
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
