import { describe, it, expect, beforeEach } from 'vitest';
import { UndoRedoManager, OperationType, TargetType, type OpRecord } from '../../src/core/undo-redo';
import type { PathEntry } from '../../src/core/path-entry';

function pe(s: string, enabled: boolean = true): PathEntry {
  return { path: s, enabled };
}

function makeRecord(type: OperationType, target: TargetType, index: number, count: number, oldPaths: PathEntry[], newPaths: PathEntry[]): OpRecord {
  return { type, target, index, count, oldPaths, newPaths };
}

describe('UndoRedoManager', () => {
  let mgr: UndoRedoManager;
  let sys: PathEntry[];
  let user: PathEntry[];

  beforeEach(() => {
    mgr = new UndoRedoManager(50);
    sys = [pe('C:\\Windows'), pe('C:\\Program Files')];
    user = [pe('C:\\Users\\me\\AppData')];
  });

  it('初始不可撤销不可重做', () => {
    expect(mgr.canUndo()).toBe(false);
    expect(mgr.canRedo()).toBe(false);
  });

  it('ADD 撤销/重做', () => {
    sys.push(pe('C:\\NewPath'));
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 2, 1, [], [pe('C:\\NewPath')]));

    const u = mgr.undo(sys, user)!;
    expect(u[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Program Files']);

    const r = mgr.redo(...u)!;
    expect(r[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Program Files', 'C:\\NewPath']);
  });

  it('DELETE 撤销/重做', () => {
    const removed = sys[0];
    mgr.push(makeRecord(OperationType.DELETE, TargetType.SYSTEM, 0, 1, [removed], []));
    sys.splice(0, 1);

    const u = mgr.undo(sys, user)!;
    expect(u[0][0].path).toBe(removed.path);

    const r = mgr.redo(...u)!;
    expect(r[0].map(e => e.path)).toEqual(['C:\\Program Files']);
  });

  it('EDIT 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.EDIT, TargetType.SYSTEM, 0, 1, [pe('C:\\Windows')], [pe('C:\\Edited')]));
    sys[0] = pe('C:\\Edited');

    const u = mgr.undo(sys, user)!;
    expect(u[0][0].path).toBe('C:\\Windows');

    const r = mgr.redo(...u)!;
    expect(r[0][0].path).toBe('C:\\Edited');
  });

  it('MOVE_UP 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.MOVE_UP, TargetType.SYSTEM, 1, 1, [], []));
    [sys[0], sys[1]] = [sys[1], sys[0]];

    const u = mgr.undo(sys, user)!;
    expect(u[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Program Files']);

    const r = mgr.redo(...u)!;
    expect(r[0].map(e => e.path)).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  it('MOVE_DOWN 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.MOVE_DOWN, TargetType.SYSTEM, 0, 1, [], []));
    [sys[0], sys[1]] = [sys[1], sys[0]];

    const u = mgr.undo(sys, user)!;
    expect(u[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Program Files']);
  });

  it('CLEAN 撤销/重做', () => {
    const old = [...sys];
    const cleaned = [pe('C:\\Windows')];
    mgr.push(makeRecord(OperationType.CLEAN, TargetType.SYSTEM, 0, 2, old, cleaned));
    sys = cleaned;

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(cleaned);
  });

  it('CLEAR 撤销/重做', () => {
    const old = [...sys];
    mgr.push(makeRecord(OperationType.CLEAR, TargetType.SYSTEM, 0, 2, old, []));
    sys = [];

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual([]);
  });

  it('IMPORT 撤销/重做', () => {
    const old = [...sys];
    const imported = [pe('C:\\New1'), pe('C:\\New2')];
    mgr.push(makeRecord(OperationType.IMPORT, TargetType.SYSTEM, 0, 2, old, imported));
    sys = imported;

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(imported);
  });

  it('新操作后截断重做分支', () => {
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], [pe('first')]));
    mgr.undo(sys, user);
    expect(mgr.canRedo()).toBe(true);
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], [pe('second')]));
    expect(mgr.canRedo()).toBe(false);
  });

  it('超出最大历史容量时移除最旧记录', () => {
    const small = new UndoRedoManager(3);
    for (let i = 0; i < 5; i++) {
      small.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], [pe(`path_${i}`)]));
    }
    expect(small.historyLength).toBe(3);
  });

  it('非连续多选 DELETE 撤销恢复到原始位置', () => {
    // 扩展初始数组
    sys.push(pe('C:\\Extra1'), pe('C:\\Extra2'));
    const old = [...sys];
    // 删除 indices [1, 3]（C:\Program Files 和 C:\Extra2）
    const removed = [sys[1], sys[3]];
    mgr.push({
      type: OperationType.DELETE, target: TargetType.SYSTEM,
      index: 1, count: 2,
      oldPaths: removed, newPaths: [],
      indices: [1, 3],
    });
    sys.splice(3, 1);
    sys.splice(1, 1);

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Extra1']);
  });

  it('操作 USER 路径', () => {
    user.push(pe('C:\\NewUserPath'));
    mgr.push(makeRecord(OperationType.ADD, TargetType.USER, 1, 1, [], [pe('C:\\NewUserPath')]));
    const u = mgr.undo(sys, user)!;
    expect(u[1].map(e => e.path)).toEqual(['C:\\Users\\me\\AppData']);
    expect(u[0].map(e => e.path)).toEqual(['C:\\Windows', 'C:\\Program Files']);
  });

  it('TOGGLE 撤销/重做', () => {
    sys[0] = pe('C:\\Windows', false);
    mgr.push(makeRecord(OperationType.TOGGLE, TargetType.SYSTEM, 0, 1,
      [pe('C:\\Windows', true)], [pe('C:\\Windows', false)]));

    const u = mgr.undo(sys, user)!;
    expect(u[0][0].enabled).toBe(true);

    const r = mgr.redo(...u)!;
    expect(r[0][0].enabled).toBe(false);
  });

  it('IMPORT_BOTH 撤销/重做（同时修改系统和用户路径）', () => {
    const oldSys = [...sys];
    const oldUser = [...user];
    const newSys = [pe('C:\\ImportedSys')];
    const newUser = [pe('C:\\ImportedUser')];

    mgr.push({
      type: OperationType.IMPORT_BOTH,
      target: TargetType.SYSTEM,
      index: 0, count: 0,
      oldPaths: oldSys, newPaths: newSys,
      oldPathsOther: oldUser, newPathsOther: newUser,
    });
    sys = newSys;
    user = newUser;

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(oldSys);
    expect(u[1]).toEqual(oldUser);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(newSys);
    expect(r[1]).toEqual(newUser);
  });
});
