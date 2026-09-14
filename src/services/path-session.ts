import i18n from '@/i18n';
import appConfig from '@/config/default.json';
import type { PathEntry } from '@/core/path-entry';
import { canWriteTarget, type PathCapabilities } from '@/core/path-capabilities';
import { TargetType } from '@/core/undo-redo';
import { backend } from '@/services/backend';

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
  pendingSys: PathEntry[] | null;
  pendingUser: PathEntry[] | null;
  isAdmin: boolean;
  capabilities: PathCapabilities;
  force?: boolean;
}

export interface SaveSnapshotOutcome {
  result: SaveResult;
  savedSys: PathEntry[];
  savedUser: PathEntry[];
  pendingSys: PathEntry[] | null;
  pendingUser: PathEntry[] | null;
  statusMessage: string;
}

type HiveSaveResult =
  | { status: 'unchanged' }
  | { status: 'success' }
  | { status: 'failure'; error: string };

export function arraysEqual(a: readonly PathEntry[], b: readonly PathEntry[]): boolean {
  return (
    a.length === b.length && a.every((v, i) => v.path === b[i].path && v.enabled === b[i].enabled)
  );
}

export function disabledLists(entries: readonly PathEntry[]): string[] {
  return entries.filter((entry) => !entry.enabled).map((entry) => entry.path);
}

/** 加载注册表与 disabled.json 合并后的完整有序快照。 */
export async function loadPathSnapshot(): Promise<LoadSnapshot> {
  const snapshot = await backend.loadPathSnapshot();
  return {
    sysEntries: snapshot.system.map((entry) => ({ ...entry })),
    usrEntries: snapshot.user.map((entry) => ({ ...entry })),
  };
}

/** 按 hive 计算并提交保存计划；禁用状态只在注册表成功后持久化。 */
export async function savePathSnapshot(request: SaveSnapshotRequest): Promise<SaveSnapshotOutcome> {
  const {
    sysPaths,
    userPaths,
    savedSys,
    savedUser,
    pendingSys,
    pendingUser,
    isAdmin,
    capabilities,
    force,
  } = request;
  const sysChanged = !arraysEqual(sysPaths, savedSys);
  const userChanged = !arraysEqual(userPaths, savedUser);

  if (!sysChanged && !userChanged && !pendingSys && !pendingUser) {
    return {
      result: { kind: 'success' },
      savedSys,
      savedUser,
      pendingSys: null,
      pendingUser: null,
      statusMessage: i18n.t('status.saved'),
    };
  }

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
      pendingSys,
      pendingUser,
      statusMessage: message,
    };
  }

  const enabledSys = sysPaths.filter((entry) => entry.enabled).map((entry) => entry.path);
  const enabledUser = userPaths.filter((entry) => entry.enabled).map((entry) => entry.path);
  const sysJoined = enabledSys.join(';');
  const userJoined = enabledUser.join(';');
  const { maxSystemLength, maxUserLength, maxCombinedLength } = appConfig.path;
  const lengthExceeded =
    (sysChanged && sysJoined.length > maxSystemLength) ||
    (userChanged && userJoined.length > maxUserLength) ||
    ((sysChanged || userChanged) && (sysJoined + userJoined).length > maxCombinedLength);
  if (!force && lengthExceeded) {
    return {
      result: { kind: 'warning', reason: 'lengthExceeded' },
      savedSys,
      savedUser,
      pendingSys,
      pendingUser,
      statusMessage: i18n.t('status.saveWarningLongPaths'),
    };
  }

  let backupFailed = false;
  if (sysChanged || userChanged) {
    await backend.backupRegistry().catch(() => {
      backupFailed = true;
    });
  }

  const origSys = savedSys.filter((entry) => entry.enabled).map((entry) => entry.path);
  const origUser = savedUser.filter((entry) => entry.enabled).map((entry) => entry.path);
  const [sysResult, userResult]: [HiveSaveResult, HiveSaveResult] = await Promise.all([
    sysChanged
      ? backend
          .saveSystemPaths(enabledSys, origSys)
          .then<HiveSaveResult>(() => ({ status: 'success' }))
          .catch<HiveSaveResult>((error) => ({ status: 'failure', error: String(error) }))
      : Promise.resolve<HiveSaveResult>({ status: 'unchanged' }),
    userChanged
      ? backend
          .saveUserPaths(enabledUser, origUser)
          .then<HiveSaveResult>(() => ({ status: 'success' }))
          .catch<HiveSaveResult>((error) => ({ status: 'failure', error: String(error) }))
      : Promise.resolve<HiveSaveResult>({ status: 'unchanged' }),
  ]);

  const sysTouched = sysResult.status === 'success';
  const userTouched = userResult.status === 'success';
  const nextSavedSys = sysTouched ? [...sysPaths] : savedSys;
  const nextSavedUser = userTouched ? [...userPaths] : savedUser;

  if (sysTouched || userTouched) {
    backend.broadcastEnvChange().catch(() => {});
  }

  // 待补写的元数据必须绑定上次已提交的注册表快照；当前草稿失败时不能覆盖它。
  let nextPendingSys = sysTouched ? [...sysPaths] : pendingSys ? [...pendingSys] : null;
  let nextPendingUser = userTouched ? [...userPaths] : pendingUser ? [...pendingUser] : null;
  let disabledStateError = '';
  if (nextPendingSys || nextPendingUser) {
    try {
      await backend.savePathSnapshot(nextPendingSys, nextPendingUser);
      nextPendingSys = null;
      nextPendingUser = null;
    } catch (error) {
      disabledStateError = String(error);
    }
  }

  const errors: string[] = [];
  if (sysResult.status === 'failure') {
    errors.push(i18n.t('status.saveSystemFailed', { error: sysResult.error }));
  }
  if (userResult.status === 'failure') {
    errors.push(i18n.t('status.saveUserFailed', { error: userResult.error }));
  }
  if (disabledStateError) {
    errors.push(`${i18n.t('status.disabledStateFailed')}: ${disabledStateError}`);
  }

  if (errors.length > 0) {
    const partial =
      sysTouched ||
      userTouched ||
      Boolean(pendingSys) ||
      Boolean(pendingUser) ||
      Boolean(disabledStateError);
    const message = i18n.t('status.saveFailure', { details: errors.join('; ') });
    return {
      result: partial ? { kind: 'partial', message } : { kind: 'failure', message },
      savedSys: nextSavedSys,
      savedUser: nextSavedUser,
      pendingSys: nextPendingSys,
      pendingUser: nextPendingUser,
      statusMessage: message,
    };
  }

  return {
    result: { kind: 'success' },
    savedSys: nextSavedSys,
    savedUser: nextSavedUser,
    pendingSys: null,
    pendingUser: null,
    statusMessage: backupFailed ? i18n.t('status.saved_without_backup') : i18n.t('status.saved'),
  };
}
