import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';
import { SearchInput } from './SearchInput';
import { ActionButtons } from './ActionButtons';
import { UndoRedoButtons } from './UndoRedoButtons';
import { ToolbarMenu } from './ToolbarMenu';
import { ExportIcon, ImportIcon, SaveIcon, SearchIcon } from '@/components/ui/icons';

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
  onSystemTheme?: () => void;
  onAnalyze: () => void;
  onProfiles: () => void;
  onRules?: () => void;
}

/** 单行分层工具栏：搜索 │ 编辑 │ 排序 │ 撤销 ...推... 批量 │ 保存 │ ⋯ */
export function ToolBar(props: ToolBarProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const isModified = useAppStore((s) => s.isModified);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <div className="toolbar" data-testid="toolbar">
      <div className="toolbar-search-wrap">
        <SearchIcon className="toolbar-search-icon" />
        <SearchInput />
      </div>
      <ActionButtons
        onNew={props.onNew}
        onEdit={props.onEdit}
        onDelete={props.onDelete}
        onMoveUp={props.onMoveUp}
        onMoveDown={props.onMoveDown}
      />
      <UndoRedoButtons />
      <div className="toolbar-right">
        <div className="toolbar-group" role="group" aria-label={t('toolbar.fileGroup')}>
          <Button disabled={!canWrite} onClick={props.onImport}>
            <ImportIcon />
            <span>{t('button.import')}</span>
          </Button>
          <Button onClick={props.onExport}>
            <ExportIcon />
            <span>{t('button.export')}</span>
          </Button>
        </div>
        {/* 未保存指示由状态栏的「● 未保存」承担；保存按钮只保留强调色。 */}
        <div className="toolbar-group" role="group" aria-label={t('toolbar.confirmGroup')}>
          <Button variant="primary" disabled={!canWrite || !isModified} onClick={props.onSave}>
            <SaveIcon />
            <span>{t('button.savePath')}</span>
          </Button>
        </div>
        <ToolbarMenu
          onBrowse={props.onBrowse}
          onClean={props.onClean}
          onAnalyze={props.onAnalyze}
          onProfiles={props.onProfiles}
          onRules={props.onRules ?? (() => {})}
          onLanguage={props.onLanguage}
          onDarkMode={props.onDarkMode}
          onSystemTheme={props.onSystemTheme ?? (() => {})}
          onHelp={props.onHelp}
          onCancel={props.onCancel}
        />
      </div>
    </div>
  );
}
