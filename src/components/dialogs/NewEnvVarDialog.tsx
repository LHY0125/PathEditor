import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useEnvStore } from '@/store/env-store';
import { validateVarName, type EnvHive, type EnvValueKind } from '@/core/env-var';

interface NewEnvVarDialogProps {
  /** 系统 hive 不可写时禁用「系统」来源（Spec 权限矩阵：非管理员不得新建系统变量）。 */
  canWriteSystem: boolean;
  /** 用户 hive 不可写时禁用「用户」来源（F-08：此前只门禁系统侧）。 */
  canWriteUser: boolean;
  onCancel: () => void;
  /** 返回是否创建成功；失败时弹窗保留并显示 store 的错误消息。 */
  onConfirm: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<boolean>;
}

const labelClass = 'w-20';
const labelStyle = { color: 'var(--app-fg-muted)' };

export function NewEnvVarDialog({
  canWriteSystem,
  canWriteUser,
  onCancel,
  onConfirm,
}: NewEnvVarDialogProps) {
  const { t } = useTranslation();
  const [hive, setHive] = useState<EnvHive>(
    canWriteUser ? 'user' : canWriteSystem ? 'system' : 'user',
  );
  const [name, setName] = useState('');
  const [value, setValue] = useState('');
  const [kind, setKind] = useState<EnvValueKind>('string');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    const invalid = validateVarName(name);
    if (invalid !== null) {
      setError(invalid);
      return;
    }
    setError(null);
    setSubmitting(true);
    // Rust 是重复变量与权限的最终裁判：失败时保留弹窗，把 store 的错误透出。
    const ok = await onConfirm(hive, name, value, kind);
    setSubmitting(false);
    if (!ok) setError(useEnvStore.getState().statusMessage);
  };

  return (
    <Modal open onClose={onCancel}>
      <div className="flex flex-col gap-2">
        <h2 className="text-base font-semibold mb-1">{t('envVar.newVarTitle')}</h2>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.source')}
          </span>
          <select
            value={hive}
            onChange={(e) => setHive(e.target.value as EnvHive)}
            className="field-input"
          >
            <option value="user" disabled={!canWriteUser}>
              {t('merge.user')}
            </option>
            <option value="system" disabled={!canWriteSystem}>
              {t('merge.system')}
            </option>
          </select>
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarName')}
          </span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="field-input flex-1"
          />
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarValue')}
          </span>
          <input
            value={value}
            onChange={(e) => setValue(e.target.value)}
            className="field-input flex-1"
          />
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarKind')}
          </span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value as EnvValueKind)}
            className="field-input"
          >
            <option value="string">String</option>
            <option value="expandString">ExpandString</option>
          </select>
        </label>

        {error && (
          <p className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        <div className="flex justify-end gap-2 mt-1">
          <Button onClick={onCancel}>{t('button.cancel')}</Button>
          <Button variant="primary" disabled={submitting} onClick={() => void submit()}>
            {t('button.save')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
