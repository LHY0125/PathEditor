import { Modal } from '@/components/ui/Modal';
import { EnvBackupPanel } from './env-backup/EnvBackupPanel';
import { useEnvBackup } from './env-backup/use-env-backup';

interface EnvBackupDialogProps {
  open: boolean;
  onClose: () => void;
  /** 恢复改的是注册表：调用方需据此刷新环境变量表格。 */
  onRestored: () => void;
}

/**
 * 环境变量备份与恢复对话框。
 *
 * 编排逻辑在 `env-backup/use-env-backup.ts`，本组件只负责挂载与关闭 ——
 * 与 `ProfileDialog` 的结构一致。
 */
export function EnvBackupDialog({ open, onClose, onRestored }: EnvBackupDialogProps) {
  const controller = useEnvBackup(open, onRestored);

  return (
    <Modal open={open} onClose={onClose}>
      <EnvBackupPanel controller={controller} />
    </Modal>
  );
}
