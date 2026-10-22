/**
 * 环境变量备份与恢复的纯展示逻辑（零 React / Tauri 依赖）。
 *
 * 差异计算、路径校验、权限判定全部在 Rust 侧完成；本模块只把结果转成
 * 用户可读的摘要，不重复实现任何判定 —— 双端各写一份规则必然产生分歧。
 */

export type RestoreChangeKind = 'added' | 'modified' | 'removed' | 'conflict';
export type EnvHive = 'system' | 'user';

/** 单条恢复差异（与 Rust `RestoreChange` 的 serde camelCase 形状一致）。 */
export interface RestoreChange {
  hive: EnvHive;
  name: string;
  kind: RestoreChangeKind;
}

/**
 * 恢复差异预览（与 Rust `RestorePreview` 一致）。
 *
 * `modified` **恒为 0**：Rust 侧 `revision_of` 对 `name + type + value` 取值，
 * 故「值变了」与「备份已过期」在差异计算里是同一个条件，后者一律判为
 * `conflict`。前端**不得**把 `conflicts` 折算成 `modified` 或重命名计数 ——
 * 那是复制核心判定逻辑。如实展示四个计数即可。
 *
 * 同理，`force = true` 覆盖冲突时被改写的那批变量仍计入 `conflicts`，
 * 因此本摘要对强制恢复**低报**实际改动量；这一取舍在前端不做补偿。
 */
export interface RestorePreview {
  changes: RestoreChange[];
  added: number;
  modified: number;
  removed: number;
  conflicts: number;
}

/** 备份文件列表项（与 Rust `EnvBackupInfo` 一致）。 */
export interface EnvBackupInfo {
  file: string;
  path: string;
  timestamp: string;
  sizeBytes: number;
  /** **恒为 0**：列表不解析内容（S5 / J4 裁断），不可当作真实计数。 */
  variableCount: number;
}

/** 恢复执行结果（与 Rust `RestoreOutcome` 一致）。 */
export interface RestoreOutcome {
  applied: number;
  /** **恒为 0**：当前没有差异类型会走到「跳过」。 */
  skipped: number;
  /** 逐条失败原因文本；空表示全部成功。 */
  failures: string[];
}

export interface BackupSummary {
  hasChanges: boolean;
  hasConflicts: boolean;
  /** 将被删除的变量名（`hive:name`）——删除最不可逆，确认弹窗必须单独高亮 */
  removedNames: string[];
  text: string;
}

/**
 * 把差异预览转成摘要。
 *
 * `hasChanges` 只看新增/修改/删除：仅有冲突时注册表一个字节都不会变
 * （默认模式下整批中止），不算「有变更」。
 */
export function summarizePreview(preview: RestorePreview): BackupSummary {
  const parts: string[] = [];
  if (preview.added > 0) parts.push(`新增 ${preview.added}`);
  if (preview.modified > 0) parts.push(`修改 ${preview.modified}`);
  if (preview.removed > 0) parts.push(`删除 ${preview.removed}`);
  if (preview.conflicts > 0) parts.push(`冲突 ${preview.conflicts}`);

  const hasChanges = preview.added + preview.modified + preview.removed > 0;
  const hasConflicts = preview.conflicts > 0;

  let text: string;
  if (parts.length === 0) {
    text = '与当前环境变量无变化';
  } else if (hasConflicts) {
    // 冲突项在 force=false 时会让恢复整体中止，必须显式说出来，
    // 否则用户会以为点「确定」一定成功。
    text = `${parts.join('、')}；其中冲突项在默认模式下会中止恢复`;
  } else {
    text = parts.join('、');
  }

  return {
    hasChanges,
    hasConflicts,
    // 带 hive 前缀：同名变量可能同时存在于两个 hive，裸名字无法区分。
    removedNames: preview.changes
      .filter((c) => c.kind === 'removed')
      .map((c) => `${c.hive}:${c.name}`),
    text,
  };
}

/** 人类可读的文件大小。 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
