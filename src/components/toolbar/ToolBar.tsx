import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';
import { SearchInput } from './SearchInput';
import { ActionButtons } from './ActionButtons';
import { UndoRedoButtons } from './UndoRedoButtons';
import { ToolbarMenu } from './ToolbarMenu';

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

/** 单行分层工具栏：搜索 │ 编辑 │ 排序 │ 撤销 ...推... 批量 │ 保存 │ ⋯ */
export function ToolBar(props: ToolBarProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <div className="toolbar" data-testid="toolbar">
      <SearchInput />
      <span className="toolbar-sep" aria-hidden="true" />
      <ActionButtons
        onNew={props.onNew}
        onEdit={props.onEdit}
        onDelete={props.onDelete}
        onMoveUp={props.onMoveUp}
        onMoveDown={props.onMoveDown}
      />
      <span className="toolbar-sep" aria-hidden="true" />
      <UndoRedoButtons />
      <div className="flex-1" />
      <Button disabled={!canWrite} onClick={props.onImport}>
        {t('button.import')}
      </Button>
      <Button onClick={props.onExport}>{t('button.export')}</Button>
      {/* 未保存指示由状态栏的「● 未保存」（StatusBar）承担：
          按钮内只有强调底色与前景文字两色，任何标记都会与其中之一同色而不可辨。 */}
      <Button variant="primary" disabled={!canWrite} onClick={props.onSave}>
        {t('button.savePath')}
      </Button>
      <ToolbarMenu
        onBrowse={props.onBrowse}
        onClean={props.onClean}
        onAnalyze={props.onAnalyze}
        onProfiles={props.onProfiles}
        onLanguage={props.onLanguage}
        onDarkMode={props.onDarkMode}
        onHelp={props.onHelp}
        onCancel={props.onCancel}
      />
    </div>
  );
}
