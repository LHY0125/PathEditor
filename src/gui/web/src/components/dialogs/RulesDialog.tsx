import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useAppStore } from '@/store/app-store';
import { backend, type Rule, type RuleApplyResult } from '@/services/backend';

interface Props {
  open: boolean;
  onClose: () => void;
}

/** PATH 整理规则：条件匹配 + 排序/剔除，可预览并写入草稿。 */
export function RulesDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<Rule[]>([]);
  const [name, setName] = useState('');
  const [contains, setContains] = useState('');
  const [action, setAction] = useState<'moveTop' | 'remove'>('moveTop');
  const [preview, setPreview] = useState<RuleApplyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    void backend
      .rulesList()
      .then(setRules)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  };

  useEffect(() => {
    if (open) load();
     
  }, [open]);

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      await backend.rulesCreate({ name, contains, action });
      setName('');
      setContains('');
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const apply = async (record: boolean) => {
    const state = useAppStore.getState();
    setBusy(true);
    setError(null);
    try {
      const result = await backend.rulesApply(state.sysPaths, state.userPaths, record);
      setPreview(result);
      if (record) {
        state.replaceBothPaths(result.system, result.user);
        onClose();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} ariaLabel={t('rules.title')}>
      <div className="flex flex-col gap-3" style={{ width: 640, maxWidth: '100%' }}>
        <h2 className="text-base font-semibold pr-8">{t('rules.title')}</h2>

        <div className="flex flex-wrap items-end gap-2 text-xs">
          <label className="flex flex-col gap-1">
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('rules.name')}</span>
            <input className="field-input" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="flex flex-col gap-1">
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('rules.contains')}</span>
            <input
              className="field-input"
              value={contains}
              onChange={(e) => setContains(e.target.value)}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span style={{ color: 'var(--app-fg-muted)' }}>{t('rules.action')}</span>
            <select
              className="field-input"
              value={action}
              onChange={(e) => setAction(e.target.value as 'moveTop' | 'remove')}
            >
              <option value="moveTop">{t('rules.moveTop')}</option>
              <option value="remove">{t('rules.remove')}</option>
            </select>
          </label>
          <Button
            size="sm"
            disabled={busy || !name.trim() || !contains.trim()}
            onClick={() => void create()}
          >
            {t('rules.add')}
          </Button>
        </div>

        {rules.length === 0 ? (
          <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('rules.empty')}
          </p>
        ) : (
          <ul className="space-y-1">
            {rules.map((rule) => (
              <li
                key={rule.uid}
                className="flex items-center gap-2 rounded border px-2 py-1 text-xs"
                style={{ borderColor: 'var(--app-border)' }}
              >
                <input
                  type="checkbox"
                  checked={rule.enabled}
                  aria-label={t('rules.enabled', { name: rule.name })}
                  onChange={(e) => {
                    void backend.rulesSetEnabled(rule.uid, e.target.checked).then(load);
                  }}
                />
                <span className="font-medium">{rule.name}</span>
                <span className="font-mono" style={{ color: 'var(--app-fg-muted)' }}>
                  {rule.action === 'moveTop' ? '↑' : '×'} {rule.condition.value}
                </span>
                <span className="ml-auto" />
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => void backend.rulesDelete(rule.uid).then(load)}
                >
                  {t('button.delete')}
                </Button>
              </li>
            ))}
          </ul>
        )}

        {preview !== null && (
          <div className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
            {t('rules.previewSummary', {
              hits: preview.hits.reduce((sum, hit) => sum + hit.matchedCount, 0),
            })}
          </div>
        )}
        {error !== null && (
          <p className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        <div className="flex justify-end gap-2">
          <Button size="sm" disabled={busy} onClick={() => void apply(false)}>
            {t('rules.preview')}
          </Button>
          <Button size="sm" variant="primary" disabled={busy} onClick={() => void apply(true)}>
            {t('rules.apply')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
