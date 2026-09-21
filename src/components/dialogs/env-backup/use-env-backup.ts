import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { backend } from '@/services/backend';
import {
  summarizePreview,
  type BackupSummary,
  type EnvBackupInfo,
  type RestoreOutcome,
  type RestorePreview,
} from '@/core/env-backup';

/**
 * 从 backend 解析层的 rejection 产物中取错误码。
 *
 * 只做形状收窄，不做判定 —— 「冲突整批中止」「未提权写不了 HKLM」这些规则
 * 都在 Rust 侧；这里读到的 code 仅用于**选择提示文案**（F-06：判定只认 code，
 * 不匹配 message 文本）。
 */
function errorCode(err: unknown): string | null {
  if (typeof err !== 'object' || err === null || !('code' in err)) return null;
  const code = (err as { code: unknown }).code;
  return typeof code === 'string' ? code : null;
}

/** 把 rejection 转成可展示文本；解析层已保证 `{code, message}`，兜底到 String()。 */
function errorText(err: unknown): string {
  if (typeof err === 'object' && err !== null && 'message' in err) {
    const message = (err as { message: unknown }).message;
    if (typeof message === 'string' && message.trim().length > 0) return message;
  }
  return String(err);
}

/** 恢复确认弹窗的完整文案（导出以便直接测试，避免断言散落在 DOM 查找里）。 */
export function buildRestoreConfirm(
  intro: string,
  summary: BackupSummary | null,
  labels: { deleteWarning: string; deleteHint: string; manualFallback: string },
): string {
  const lines = [intro];
  if (summary) {
    lines.push('', summary.text);
    // 删除是最不可逆的部分：变量名必须逐个列出，不能只给计数。
    if (summary.removedNames.length > 0) {
      lines.push(
        '',
        labels.deleteWarning,
        ...summary.removedNames.map((name) => `  • ${name}`),
        labels.deleteHint,
      );
    }
  }
  // 恢复不产生新备份（设计文档 C7）：手工兜底出口必须在确认前给到。
  lines.push('', labels.manualFallback, '  patheditor backup', '  patheditor env backup');
  return lines.join('\n');
}

export interface EnvBackupController {
  /** 备份列表；`null` 表示尚未加载完成（区别于「已加载但为空列表」）。 */
  backups: EnvBackupInfo[] | null;
  /** 预览或恢复进行中：期间禁用会触发 IPC 的按钮，避免并发恢复。 */
  busy: boolean;
  creating: boolean;
  selected: EnvBackupInfo | null;
  preview: RestorePreview | null;
  summary: BackupSummary | null;
  outcome: RestoreOutcome | null;
  /** 「立即备份」成功后返回的路径，供状态区展示（**不保证绝对路径**）。 */
  createdPath: string | null;
  error: string | null;
  createBackup: () => Promise<void>;
  select: (info: EnvBackupInfo) => Promise<void>;
  /** 默认模式恢复；遇 code=`conflict` 时二次确认并以 `force=true` 重试一次。 */
  restore: () => Promise<void>;
}

/**
 * 备份与恢复对话框的状态机（IPC 编排 + 确认流程）。
 *
 * 抽成 hook 而不是塞进组件：确认弹窗与错误分支的先后顺序是本特性的核心
 * 行为，独立于渲染即可被测试覆盖。
 *
 * `onRestored` 由调用方提供（刷新环境变量表格）：恢复改的是注册表，
 * 列表必须换代，否则界面与注册表不一致。
 */
export function useEnvBackup(open: boolean, onRestored: () => void): EnvBackupController {
  const { t } = useTranslation();
  // null = 尚未加载：比布尔 loading 更诚实（加载失败时不会一直显示「加载中」）。
  const [backups, setBackups] = useState<EnvBackupInfo[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [selected, setSelected] = useState<EnvBackupInfo | null>(null);
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [outcome, setOutcome] = useState<RestoreOutcome | null>(null);
  const [createdPath, setCreatedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 供事件处理器复用（「立即备份」后刷新列表）。刻意**不**在 effect 里调用它：
  // 它的首条 setState 前面只有 await，被 effect 同步调用会触发级联渲染
  // （react-hooks/set-state-in-effect），与 use-analyze-data.ts 的写法保持一致。
  const refresh = useCallback(async () => {
    try {
      const list = await backend.listEnvBackups();
      setBackups(list);
    } catch (err: unknown) {
      setError(errorText(err));
    }
  }, []);

  // 仅在打开时拉列表；关闭期间不打扰后端。卸载/关闭后晚到的响应被丢弃。
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    backend
      .listEnvBackups()
      .then((list) => {
        if (!cancelled) setBackups(list);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(errorText(err));
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  const createBackup = useCallback(async () => {
    setCreating(true);
    setError(null);
    try {
      const path = await backend.backupEnvVars();
      setCreatedPath(path);
      await refresh();
    } catch (err: unknown) {
      setError(errorText(err));
    } finally {
      setCreating(false);
    }
  }, [refresh]);

  const select = useCallback(
    async (info: EnvBackupInfo) => {
      setSelected(info);
      setPreview(null);
      setOutcome(null);
      setCreatedPath(null);
      setError(null);
      setBusy(true);
      try {
        // 路径来源校验（扩展名 / 备份目录 / 大小上限）由 Rust 的
        // preview_env_backup 在读取**之前**完成 —— 前端不复制该规则。
        setPreview(await backend.previewEnvBackup(info.path));
      } catch (err: unknown) {
        setError(
          errorCode(err) === 'permissionDenied' ? t('envBackup.adminRequired') : errorText(err),
        );
        // 预览失败时清掉选中，避免「恢复」按钮对着没有差异的备份可用。
        setSelected(null);
      } finally {
        setBusy(false);
      }
    },
    [t],
  );

  /**
   * 执行恢复：默认模式一次；遇 code=`conflict` 时二次确认后**以 force=true 重试一次**。
   *
   * 刻意写成线性两步而不是递归 —— 递归自引用会触发 `react-hooks/immutability`
   * （访问尚未声明的变量），且「最多重试一次」本来就是契约。
   */
  const runRestore = useCallback(
    async (info: EnvBackupInfo, force: boolean) => {
      setBusy(true);
      setError(null);
      try {
        let result: RestoreOutcome;
        try {
          result = await backend.restoreEnvBackup(info.path, force);
        } catch (err: unknown) {
          if (errorCode(err) !== 'conflict' || force) throw err;
          // 冲突：默认模式下整批中止且注册表零改动，可以安全地再问一次。
          const ok = await backend
            .confirmDialog(
              [
                t('envBackup.conflictTitle'),
                '',
                t('envBackup.conflictHelp'),
                '',
                t('envBackup.forceConfirm'),
              ].join('\n'),
            )
            .catch(() => false);
          // 用户拒绝强制覆盖：这不是错误，静默回到就绪态。
          if (!ok) return;
          result = await backend.restoreEnvBackup(info.path, true);
        }
        setOutcome(result);
        setPreview(null);
        onRestored();
      } catch (err: unknown) {
        setError(
          errorCode(err) === 'permissionDenied' ? t('envBackup.adminRequired') : errorText(err),
        );
      } finally {
        setBusy(false);
      }
    },
    [t, onRestored],
  );

  const restore = useCallback(async () => {
    if (!selected) return;
    // 恢复不可逆且**不产生新备份**：确认文案里必须同时给出差异摘要、
    // 将被删除的变量名，以及手工兜底出口（spec §手工兜底出口）。
    const text = buildRestoreConfirm(t('envBackup.confirmRestore'), summaryOf(preview), {
      deleteWarning: t('envBackup.deleteWarning'),
      deleteHint: t('envBackup.deleteWarningHint'),
      manualFallback: t('envBackup.manualFallback'),
    });
    // 破坏性操作：对话框 IPC 失败按「取消」处理（fail-closed）。
    const confirmed = await backend.confirmDialog(text).catch(() => false);
    if (!confirmed) return;
    await runRestore(selected, false);
  }, [selected, preview, t, runRestore]);

  return {
    backups,
    busy,
    creating,
    selected,
    preview,
    summary: summaryOf(preview),
    outcome,
    createdPath,
    error,
    createBackup,
    select,
    restore,
  };
}

/** `preview` 为 null 时摘要也为 null（区别于「无变化」的空摘要）。 */
function summaryOf(preview: RestorePreview | null): BackupSummary | null {
  return preview === null ? null : summarizePreview(preview);
}
