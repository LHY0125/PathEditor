#!/usr/bin/env node
/**
 * 仓库一致性检查：把「同一个数字写在多处」的地方收敛成可验证的断言。
 *
 * 覆盖四类漂移（都曾在仓库里真实发生）：
 * 1. 版本号四处不一致（package.json / Cargo.toml / tauri.conf.json / README 徽章）；
 * 2. `CURRENT_SCHEMA_VERSION` 与文档描述不一致；
 * 3. 测试基线数字与 `docs/dev/testing.md` 的权威值不一致；
 * 4. `CLAUDE.md` 与 `AGENTS.md` 内容不同步。
 *
 * 用法：`node scripts/check-consistency.mjs`（无外部依赖，CI 与本地通用）。
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (rel) => readFileSync(join(root, rel), 'utf8');
const failures = [];
const check = (ok, message) => {
  if (!ok) failures.push(message);
};

// ── 1. 版本号 ────────────────────────────────────────────────
const pkgVersion = JSON.parse(read('package.json')).version;
const cargoVersion = read('Cargo.toml').match(
  /\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
)?.[1];
const tauriVersion = JSON.parse(read('src/gui/src-tauri/tauri.conf.json')).version;
const badgeVersion = read('README.md').match(/badge\/version-([\d.]+)-/)?.[1];

check(
  [cargoVersion, tauriVersion, badgeVersion].every((v) => v === pkgVersion),
  `版本号不一致：package.json=${pkgVersion} Cargo.toml=${cargoVersion} ` +
    `tauri.conf.json=${tauriVersion} README徽章=${badgeVersion}`,
);

// ── 2. schema 版本 ──────────────────────────────────────────
const schemaVersion = read('src/core/src/db/schema.rs').match(
  /CURRENT_SCHEMA_VERSION:\s*u32\s*=\s*(\d+)/,
)?.[1];
check(Boolean(schemaVersion), '无法从 schema.rs 解析 CURRENT_SCHEMA_VERSION');
for (const doc of ['CLAUDE.md', 'AGENTS.md', 'docs/dev/sqlite-storage.md']) {
  const found = read(doc).match(/CURRENT_SCHEMA_VERSION\s*=\s*(\d+)/g) ?? [];
  for (const hit of found) {
    check(
      hit.match(/(\d+)/)?.[1] === schemaVersion,
      `${doc} 的 schema 版本与 schema.rs（${schemaVersion}）不一致：${hit}`,
    );
  }
}

// ── 3. 测试基线 ─────────────────────────────────────────────
const testing = read('docs/dev/testing.md');
const baseline = testing.match(/Rust \*\*(\d+) passed[^*]*\*\*/)?.[1];
check(Boolean(baseline), '无法从 docs/dev/testing.md 解析 Rust 基线');
if (baseline) {
  for (const doc of ['README.md', 'CLAUDE.md', 'AGENTS.md']) {
    const text = read(doc);
    const numbers = [...text.matchAll(/Rust[^\n]*?(\d+) passed/g)].map((m) => m[1]);
    for (const n of numbers) {
      check(n === baseline, `${doc} 的 Rust 基线 ${n} 与 testing.md（${baseline}）不一致`);
    }
  }
  const badge = read('README.md').match(/badge\/tests-(\d+)%20passed/)?.[1];
  const frontend = testing.match(/前端 \*\*(\d+) files \/ (\d+) passed\*\*/);
  if (badge && frontend) {
    const expected = Number(baseline) + Number(frontend[2]);
    check(
      Number(badge) === expected,
      `README 测试徽章 ${badge} 应等于 Rust ${baseline} + 前端 ${frontend[2]} = ${expected}`,
    );
  }
}

// ── 4. 指令文件同步 ─────────────────────────────────────────
check(
  read('CLAUDE.md') === read('AGENTS.md'),
  'CLAUDE.md 与 AGENTS.md 内容不一致（两者应逐字节相同）',
);

// ── 结果 ────────────────────────────────────────────────────
if (failures.length > 0) {
  console.error('一致性检查失败：');
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('一致性检查通过（版本 / schema / 基线 / 指令文件）');
