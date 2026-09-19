import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { useEnvStore } from '@/store/env-store';
import { findMetaByKey } from '@/core/env-var';

interface EditEnvVarDialogProps {
  /**
   * 稳定键（`${hive}:${name}`）。meta 不随 prop 固化 —— 每次提交前由
   * 调用方从最新快照派生，冲突刷新后重试自动携带新 revision（F-02）。
   */
  varKey: string;
  onCancel: () => void;
  /**
   * 返回是否保存成功；失败时弹窗保留并显示 store 的错误消息。
   * readRevision 是弹窗读取原值时的 revision，供保存点陈旧判定（F-01）。
   */
  onConfirm: (value: string, readRevision: string | null) => Promise<boolean>;
}

/**
 * 编辑环境变量值的弹窗。
 *
 * - 打开时经 `fetchFullValue` 取**完整原值**：preview 是截断/净化后的展示
 *   摘要，严禁作为编辑数据源，否则原样保存也会损坏长值/含换行的值（F-01）。
 * - 输入实时同步进 store 草稿：真实输入参与关窗确认，X/Alt+F4 不会静默丢弃（F-04）。
 * - 草稿策略统一为「镜像弹窗输入」：冲突后保留（输入不丢、revision 已随
 *   快照刷新），成功或取消时清除（c2）。
 */
export function EditEnvVarDialog({ varKey, onCancel, onConfirm }: EditEnvVarDialogProps) {
  const { t } = useTranslation();
  const setDraftByKey = useEnvStore((s) => s.setDraftByKey);
  const clearDraftByKey = useEnvStore((s) => s.clearDraftByKey);
  // 每次提交前由调用方从最新快照派生 meta；此处派生仅用于展示与禁用态。
  const meta = useEnvStore((s) => (s.snapshot ? findMetaByKey(s.snapshot, varKey) : null));
  const gone = meta === null;
  const [value, setValue] = useState<string | null>(null); // null = 加载完整原值中
  // 读取原值时的 revision（F-01）：提交时与最新快照比对，判断值是否已陈旧。
  const [readRevision, setReadRevision] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [name] = useState(
    () =>
      findMetaByKey(
        useEnvStore.getState().snapshot ?? { system: [], user: [], capturedAt: 0 },
        varKey,
      )?.name ?? varKey,
  );

  // 打开时取完整原值；仅在打开/换键时取一次。
  useEffect(() => {
    const store = useEnvStore.getState();
    const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
    if (!current) return;
    let cancelled = false;
    store
      .fetchFullValue(current.hive, current.name)
      .then((full) => {
        if (cancelled) return;
        setValue(full.value);
        setReadRevision(full.revision);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(String(err));
          setValue('');
        }
      });
    return () => {
      cancelled = true;
    };
  }, [varKey]);

  const submit = async () => {
    if (value === null) return;
    setSubmitting(true);
    // F-01：提交前若快照 revision 已变（外部修改），先重取最新完整值，
    // 本次不保存；用户看到新值后可再次确认。
    const store = useEnvStore.getState();
    const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
    if (current && readRevision !== null && current.revision !== readRevision) {
      // 重取是异步 IPC，可能被拒绝（变量被外部删除/类型转 Unsupported）；
      // 必须容错，否则 rejection 会让 setSubmitting 永不执行，按钮永久禁用。
      try {
        const fresh = await store.fetchFullValue(current.hive, current.name);
        setValue(fresh.value);
        setReadRevision(fresh.revision);
        setDraftByKey(varKey, fresh.value);
        setError(t('envVar.staleReloaded'));
      } catch (err: unknown) {
        setError(String(err));
      }
      setSubmitting(false);
      return;
    }
    const ok = await onConfirm(value, readRevision);
    setSubmitting(false);
    if (!ok) setError(useEnvStore.getState().statusMessage);
    // 失败不清草稿（c2 统一策略）：草稿镜像输入，供重试与关窗确认。
  };

  return (
    <Modal open onClose={onCancel}>
      <div className="flex flex-col gap-2 text-sm">
        <h2 className="text-base font-semibold mb-1">{t('envVar.editVarTitle')}</h2>

        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarName')}</span>
          <span className="flex-1 font-mono" title={name}>
            {name}
          </span>
        </label>

        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarValue')}</span>
          <input
            autoFocus
            disabled={value === null}
            value={value ?? ''}
            placeholder={value === null ? t('status.loading') : ''}
            onChange={(e) => {
              setValue(e.target.value);
              // 实时镜像进草稿：真实输入纳入关窗确认（F-04）
              setDraftByKey(varKey, e.target.value);
            }}
            className="flex-1 px-2 py-0.5 rounded border font-mono"
            style={{ backgroundColor: 'var(--app-list-bg)', borderColor: 'var(--app-border)' }}
          />
        </label>

        {gone && <p className="text-yellow-500 text-xs">{t('envVar.varGone')}</p>}
        {error && <p className="text-red-500 text-xs">{error}</p>}

        <div className="flex justify-end gap-2 mt-1">
          <button
            className="px-4 py-1.5 text-sm rounded border"
            style={{ borderColor: 'var(--app-border)' }}
            onClick={() => {
              clearDraftByKey(varKey);
              onCancel();
            }}
          >
            {t('button.cancel')}
          </button>
          <button
            className="px-4 py-1.5 text-sm rounded text-white"
            style={{ backgroundColor: '#2563eb' }}
            onClick={() => void submit()}
            disabled={submitting || value === null || meta === null}
          >
            {t('button.save')}
          </button>
        </div>
      </div>
    </Modal>
  );
}
