import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { describe, it, expect } from 'vitest';

/**
 * IPC 契约差集：`invoke_handler!` 注册的命令与 `backend/` 实际调用的命令必须一一对应。
 *
 * 两侧都从源码静态提取，不依赖运行期 mock：
 * - Rust 侧读 `invoke_handler([...])` 里的 `commands::<module>::<fn>`；
 *   命令名即函数名（全部是裸 `#[tauri::command]`，没有 `rename`）。
 * - 前端侧读 `services/backend/*.ts` 里的 `invoke('name', ...)` 字面量，
 *   以及 `invoke(identifier)` 的 `const identifier = 'name'` 绑定。
 *
 * 两个方向的差集都必须为空：
 * - 只注册不调用 → 死命令或前端漏接；
 * - 只调用不注册 → 运行时必然报 "command not found"。
 */

/** 从当前工作目录向上找仓库根（以 Rust handler 文件为锚点）。 */
function findRepoRoot(start: string): string {
  let dir = start;
  for (let i = 0; i < 8; i += 1) {
    if (existsSync(join(dir, 'src/gui/src-tauri/src/lib.rs'))) {
      return dir;
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  throw new Error(`无法从 ${start} 向上定位仓库根（找不到 src/gui/src-tauri/src/lib.rs）`);
}

const repoRoot = findRepoRoot(process.cwd());

/** 从 `lib.rs` 的 `invoke_handler!` 列表提取已注册命令名。 */
function registeredCommands(): Set<string> {
  const source = readFileSync(join(repoRoot, 'src/gui/src-tauri/src/lib.rs'), 'utf8');
  const start = source.indexOf('.invoke_handler(');
  const end = source.indexOf('.run(', start);
  if (start < 0 || end < 0) {
    throw new Error('lib.rs 中找不到 invoke_handler 或 run 调用');
  }
  const block = source.slice(start, end);
  const names = [...block.matchAll(/commands::\w+::(\w+)/g)].map((m) => m[1]);
  if (names.length === 0) {
    throw new Error('invoke_handler 列表为空，解析规则可能已失效');
  }
  return new Set(names);
}

/** 从前端 backend 目录提取所有会被 invoke 的命令名。 */
function invokedCommands(): Set<string> {
  const dir = join(repoRoot, 'src/gui/web/src/services/backend');
  const files = readdirSync(dir).filter((name) => name.endsWith('.ts'));
  const invoked = new Set<string>();

  for (const file of files) {
    const source = readFileSync(join(dir, file), 'utf8');

    // 1) 字面量形式：invoke('name', ...) / invoke<T>('name', ...)
    for (const m of source.matchAll(/invoke(?:<[^>]*>)?\(\s*(['"])([^'"]+)\1/g)) {
      invoked.add(m[2]);
    }

    // 2) 标识符形式：invoke(identifier, ...)，要求同一文件里有唯一的 const 绑定
    const bindings = new Map<string, Set<string>>();
    for (const m of source.matchAll(/const\s+([A-Za-z_$][\w$]*)\s*=\s*(['"])([^'"]+)\2/g)) {
      const set = bindings.get(m[1]) ?? new Set<string>();
      set.add(m[3]);
      bindings.set(m[1], set);
    }
    for (const m of source.matchAll(/invoke(?:<[^>]*>)?\(\s*([A-Za-z_$][\w$]*)\s*[,)]/g)) {
      const literal = bindings.get(m[1]);
      if (!literal || literal.size !== 1) {
        throw new Error(
          `${file} 中的 invoke(${m[1]}) 不是唯一字符串常量；` +
            '差集测试无法静态解析，请改为字面量命令名',
        );
      }
      invoked.add([...literal][0]);
    }
  }

  if (invoked.size === 0) {
    throw new Error('backend 目录中未解析出任何 invoke 命令，解析规则可能已失效');
  }
  return invoked;
}

/** 递归收集目录下的 `.ts` / `.tsx` 文件。 */
function walkTs(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      walkTs(full, out);
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

/** 从 `backend` 对象字面量提取方法名（缩进两格的键）。 */
function backendMethods(): Set<string> {
  const source = readFileSync(join(repoRoot, 'src/gui/web/src/services/backend/index.ts'), 'utf8');
  const start = source.indexOf('export const backend = {');
  const end = source.indexOf('\n};', start);
  if (start < 0 || end < 0) {
    throw new Error('backend 对象字面量解析失败');
  }
  const block = source.slice(start, end);
  const methods = [...block.matchAll(/^ {2}([A-Za-z_$][\w$]*):/gm)].map((m) => m[1]);
  if (methods.length === 0) {
    throw new Error('backend 对象中未解析出任何方法，解析规则可能已失效');
  }
  return new Set(methods);
}

/** 收集 `src/` 中所有 `backend.<method>` 的调用点（不含 backend 目录自身）。 */
function calledBackendMethods(): Set<string> {
  const backendDir = join(repoRoot, 'src/gui/web/src/services/backend');
  const used = new Set<string>();
  for (const file of walkTs(join(repoRoot, 'src/gui/web/src'))) {
    if (file.startsWith(backendDir)) continue;
    const source = readFileSync(file, 'utf8');
    // 允许链式换行：`backend\n  .method(...)`。
    for (const m of source.matchAll(/\bbackend\s*\.\s*([A-Za-z_$][\w$]*)/g)) {
      used.add(m[1]);
    }
  }
  return used;
}

describe('IPC 契约', () => {
  it('invoke_handler 与 backend 调用的命令集合完全一致', () => {
    const registered = registeredCommands();
    const invoked = invokedCommands();

    const deadCommands = [...registered].filter((name) => !invoked.has(name)).sort();
    const missingCommands = [...invoked].filter((name) => !registered.has(name)).sort();

    expect(
      { 只注册未调用: deadCommands, 只调用未注册: missingCommands },
      'IPC 差集必须为空；新增命令时同步 lib.rs 与 backend/',
    ).toEqual({ 只注册未调用: [], 只调用未注册: [] });
  });

  it('backend 的每个方法都有业务调用者', () => {
    // 上一轮测「注册 ↔ 包装」一致；这里补另一半：
    // 包装存在但无人调用 = 死命令（例如曾经的 profile_mark_applied）。
    const methods = backendMethods();
    const used = calledBackendMethods();
    const dead = [...methods].filter((name) => !used.has(name)).sort();

    expect(dead, 'backend 包装必须有业务调用者；新增死包装请删除').toEqual([]);
  });
});
