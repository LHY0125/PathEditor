import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

interface EditEnvVarDialogProps {
  meta: EnvVarMeta;
  onCancel: () => void;
  /** 返回是否保存成功；失败时弹窗保留并显示 store 的错误消息。 */
  onConfirm: (value: string) => Promise<boolean>;
}

/** 编辑弹窗初始值：敏感变量已被 reveal 时用明文，否则用 preview。 */
function editInitialValue(meta: EnvVarMeta, revealed: Map<string, string>): string {
  return revealed.get(`${meta.hive}:${meta.name}`) ?? meta.preview ?? '';
}

/**
 * 编辑环境变量值的弹窗。
 *
 * 变量名不可改（Spec：改名暂不支持）；草稿的写入与提交由调用方
 * （AppShell）编排，revision 冲突等失败会保留弹窗并显示错误。
 */
export function EditEnvVarDialog({ meta, onCancel, onConfirm }: EditEnvVarDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(() => editInitialValue(meta, useEnvStore.getState().revealed));
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    setSubmitting(true);
    const ok = await onConfirm(value);
    setSubmitting(false);
    if (!ok) setError(useEnvStore.getState().statusMessage);
  };

  return (
    <Modal open onClose={onCancel}>
      <div className="flex flex-col gap-2 text-sm">
        <h2 className="text-base font-semibold mb-1">{t('envVar.editVarTitle')}</h2>

        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarName')}</span>
          <span className="flex-1 font-mono" title={meta.name}>
            {meta.name}
          </span>
        </label>

        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarValue')}</span>
          <input
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            className="flex-1 px-2 py-0.5 rounded border font-mono"
            style={{ backgroundColor: 'var(--app-list-bg)', borderColor: 'var(--app-border)' }}
          />
        </label>

        {error && <p className="text-red-500 text-xs">{error}</p>}

        <div className="flex justify-end gap-2 mt-1">
          <button
            className="px-4 py-1.5 text-sm rounded border"
            style={{ borderColor: 'var(--app-border)' }}
            onClick={onCancel}
          >
            {t('button.cancel')}
          </button>
          <button
            className="px-4 py-1.5 text-sm rounded text-white"
            style={{ backgroundColor: '#2563eb' }}
            onClick={() => void submit()}
            disabled={submitting}
          >
            {t('button.save')}
          </button>
        </div>
      </div>
    </Modal>
  );
}
