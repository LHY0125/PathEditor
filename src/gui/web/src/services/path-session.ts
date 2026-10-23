import i18n from '@/i18n';
import appConfig from '@/config/default.json';
import type { PathEntry } from '@/core/path-entry';
import { canWriteTarget, type PathCapabilities } from '@/core/path-capabilities';
import { TargetType } from '@/core/undo-redo';
import { backend, type ApplyOutcome } from '@/services/backend';

/** 从 `parseCoreError` 的 `{code, message}` 或普通异常中取可读文案。 */
function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

export type SaveResult =
  | { kind: 'success' }
  | { kind: 'warning'; reason: 'lengthExceeded' }
  | { kind: 'failure'; message: string }
  | { kind: 'partial'; message: string }
  | { kind: 'blocked' };

interface LoadSnapshot {
  sysEntries: PathEntry[];
  usrEntries: PathEntry[];
}

export interface SaveSnapshotRequest {
  sysPaths: PathEntry[];
  userPaths: PathEntry[];
  savedSys: PathEntry[];
  savedUser: PathEntry[];
  isAdmin: boolean;
  capabilities: PathCapabilities;
  force?: boolean;
}

export interface SaveSnapshotOutcome {
  result: SaveResult;
  savedSys: PathEntry[];
  savedUser: PathEntry[];
  statusMessage: string;
}

export function arraysEqual(a: readonly PathEntry[], b: readonly PathEntry[]): boolean {
  return (
    a.length === b.length && a.every((v, i) => v.path === b[i].path && v.enabled === b[i].enabled)
  );
}

export function disabledLists(entries: readonly PathEntry[]): string[] {
  return entries.filter((entry) => !entry.enabled).map((entry) => entry.path);
}

/** 加载注册表与 DB 当前快照合并后的完整有序快照。 */
export async function loadPathSnapshot(): Promise<LoadSnapshot> {
  const snapshot = await backend.loadPathSnapshot();
  return {
    sysEntries: snapshot.system.map((entry) => ({ ...entry })),
    usrEntries: snapshot.user.map((entry) => ({ ...entry })),
  };
}

/** 按 hive 计算并提交保存计划；禁用状态只在注册表成功后持久化。
 *
 * -B：编排已下沉到 `core::service::save_path_with_sidecar`
 * （经 `backend.savePathWithSidecar`）。本函数**只保留三件 core 表达不了的事**：
 * 1. 权限预检（`kind: 'blocked'`，来自 `canWriteTarget`）；
 * 2. 组合长度软提醒（`kind: 'warning'`，可 `force` 放行）；
 * 3. `ApplyOutcome` → `SaveSnapshotOutcome` 的文案映射。
 *
 * 已**删除**（改由 core 承担）：自理写前备份、两 hive 并行写、store 内 pending。
 * 签名与返回类型**保持不变**，`app-store.ts` 调用点无需改动。
 *
 * **pending 语义**：pending 统一由 core 的磁盘文件机制处理
 * （`disabled::save_pending_path_snapshot` + `retry_pending_path_state`），
 * 本函数不携带 pending 字段。
 */
export async function savePathSnapshot(request: SaveSnapshotRequest): Promise<SaveSnapshotOutcome> {
  const { sysPaths, userPaths, savedSys, savedUser, isAdmin, capabilities, force } = request;
  const sysChanged = !arraysEqual(sysPaths, savedSys);
  const userChanged = !arraysEqual(userPaths, savedUser);

  if (!sysChanged && !userChanged) {
    return {
      result: { kind: 'success' },
      savedSys,
      savedUser,
      statusMessage: i18n.t('status.saved'),
    };
  }

  // ── 1. 权限预检（core 表达不了：依赖运行时的 PathCapabilities）──
  const canWriteSys = canWriteTarget(isAdmin, capabilities, TargetType.SYSTEM);
  const canWriteUser = canWriteTarget(isAdmin, capabilities, TargetType.USER);
  const permissionErrors: string[] = [];
  if (sysChanged && !canWriteSys) permissionErrors.push(i18n.t('status.noSystemPermission'));
  if (userChanged && !canWriteUser) permissionErrors.push(i18n.t('status.noUserPermission'));
  if (permissionErrors.length > 0) {
    const message = permissionErrors.join('; ');
    return {
      result: { kind: 'failure', message },
      savedSys,
      savedUser,
      statusMessage: message,
    };
  }

  // ── 2. 组合长度软提醒──
  const enabledSys = sysPaths.filter((entry) => entry.enabled).map((entry) => entry.path);
  const enabledUser = userPaths.filter((entry) => entry.enabled).map((entry) => entry.path);
  const sysJoined = enabledSys.join(';');
  const userJoined = enabledUser.join(';');
  const { maxCombinedLength } = appConfig.path;
  const lengthExceeded =
    (sysChanged || userChanged) && (sysJoined + userJoined).length > maxCombinedLength;
  if (!force && lengthExceeded) {
    return {
      result: { kind: 'warning', reason: 'lengthExceeded' },
      savedSys,
      savedUser,
      statusMessage: i18n.t('status.saveWarningLongPaths'),
    };
  }

  // ── 3. 交给 core 唯一编排（备份 + 并发 + 顺序写 + 广播 + sidecar）──
  // `expected` 取上次已保存的**启用**路径（乐观并发校验）。
  const origSys = savedSys.filter((entry) => entry.enabled).map((entry) => entry.path);
  const origUser = savedUser.filter((entry) => entry.enabled).map((entry) => entry.path);

  let outcome;
  try {
    outcome = await backend.savePathWithSidecar({
      system: sysChanged ? { entries: sysPaths, expected: origSys } : null,
      user: userChanged ? { entries: userPaths, expected: origUser } : null,
    });
  } catch (error) {
    return {
      result: { kind: 'failure', message: errorMessage(error) },
      savedSys,
      savedUser,
      statusMessage: i18n.t('status.saveFailure', { details: errorMessage(error) }),
    };
  }

  // ── 4. 映射 ApplyOutcome → SaveSnapshotOutcome ──
  //
  // 两类问题分开处理：
  // - **hive 失败**（注册表未写入）→ 进 `errors`，把结果降级为 failure/partial；
  // - **sidecar 异常**（注册表**已写入**，仅快照未落盘）→ **非阻断提示**：
  //  不降级结果，只改 `statusMessage`。core 会在下次命令时经
  //  `retry_pending_path_state` 自动补写。静默持久化失败是回归，故必须提示。
  const errors: string[] = [];
  const sysApplied = hiveApplied(outcome.system);
  const userApplied = hiveApplied(outcome.user);
  if (hiveFailure(outcome.system)) {
    errors.push(i18n.t('status.saveSystemFailed', { error: hiveFailure(outcome.system)! }));
  }
  if (hiveFailure(outcome.user)) {
    errors.push(i18n.t('status.saveUserFailed', { error: hiveFailure(outcome.user)! }));
  }
  const sidecarProblem = sidecarMessage(outcome.sidecar);

  const nextSavedSys = sysApplied ? [...sysPaths] : savedSys;
  const nextSavedUser = userApplied ? [...userPaths] : savedUser;

  if (errors.length > 0) {
    const partial = sysApplied || userApplied;
    const message = i18n.t('status.saveFailure', { details: errors.join('; ') });
    return {
      result: partial ? { kind: 'partial', message } : { kind: 'failure', message },
      savedSys: nextSavedSys,
      savedUser: nextSavedUser,
      statusMessage: message,
    };
  }

  // 注册表侧全部成功。若 sidecar 异常，结果仍是 success，但状态栏**必须**提示
  // （不能静默：用户应知道「注册表已生效、快照未落盘，下次自动补写」）。
  const statusMessage = sidecarProblem
    ? `${i18n.t('status.saved')} — ${i18n.t('status.disabledStateFailed')}: ${sidecarProblem}`
    : i18n.t('status.saved');

  return {
    result: { kind: 'success' },
    savedSys: nextSavedSys,
    savedUser: nextSavedUser,
    statusMessage,
  };
}

/** 该 hive 是否成功写入（`ApplyOutcome` 的 `applied`）。 */
function hiveApplied(outcome: ApplyOutcome['system']): boolean {
  return outcome === 'applied';
}

/** 该 hive 的失败文案（非 `failed` 时返回 `null`）。 */
function hiveFailure(outcome: ApplyOutcome['system']): string | null {
  return typeof outcome === 'object' && 'failed' in outcome ? outcome.failed.message : null;
}

/** sidecar 异常文案（`saved` 时返回 `null`）。 */
function sidecarMessage(outcome: ApplyOutcome['sidecar']): string | null {
  if (outcome === 'saved') return null;
  if ('pending' in outcome) return outcome.pending.message;
  return outcome.failed.message;
}
