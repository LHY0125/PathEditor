import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';
import { Menu } from '@/components/ui/Menu';

interface ToolbarMenuProps {
  onBrowse: () => void;
  onClean: () => void;
  onAnalyze: () => void;
  onProfiles: () => void;
  onLanguage: () => void;
  onDarkMode: () => void;
  onHelp: () => void;
  onCancel: () => void;
}

/** 工具栏 ⋯ 菜单：收纳低频操作。高频编辑动作留在主行。 */
export function ToolbarMenu(props: ToolbarMenuProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const isDark = useThemeStore((s) => s.isDark);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <Menu
      triggerAriaLabel={t('menu.label')}
      groups={[
        {
          key: 'maintenance',
          label: t('menu.maintenance'),
          items: [
            // 浏览与新建同源（open directory + addPath），高频度低于新建，故收纳于此
            {
              key: 'browse',
              label: t('button.browse'),
              onSelect: props.onBrowse,
              disabled: !canWrite,
            },
            { key: 'clean', label: t('menu.clean'), onSelect: props.onClean, disabled: !canWrite },
            { key: 'analyze', label: t('menu.analyze'), onSelect: props.onAnalyze },
            { key: 'profiles', label: t('menu.profiles'), onSelect: props.onProfiles },
          ],
        },
        {
          key: 'application',
          label: t('menu.application'),
          items: [
            // 深/浅双向切换：文案随当前状态显示「将要切到」的目标
            {
              key: 'theme',
              label: isDark ? t('button.lightMode') : t('button.darkMode'),
              onSelect: props.onDarkMode,
            },
            { key: 'lang', label: t('menu.language'), onSelect: props.onLanguage },
            { key: 'help', label: t('menu.help'), onSelect: props.onHelp, shortcut: 'F1' },
          ],
        },
      ]}
      footer={{
        key: 'cancel',
        label: t('button.cancelAndClose'),
        onSelect: props.onCancel,
        danger: true,
      }}
    />
  );
}
