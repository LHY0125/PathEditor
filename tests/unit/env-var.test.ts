import { describe, it, expect } from 'vitest';
import {
  displayValue,
  envVarKey,
  filterEnvVars,
  maskValue,
  validateVarName,
  type EnvVarMeta,
  type EnvVarSnapshot,
} from '@/core/env-var';

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

describe('envVarKey', () => {
  it('按 hive 与名称组合，区分同名跨 hive 变量', () => {
    expect(envVarKey(meta({ hive: 'user', name: 'PSModulePath' }))).toBe('user:PSModulePath');
    expect(envVarKey(meta({ hive: 'system', name: 'PSModulePath' }))).toBe('system:PSModulePath');
  });
});

describe('maskValue', () => {
  it('返回固定占位符，不泄露真实长度', () => {
    expect(maskValue()).toBe('••••••••');
    expect(maskValue()).toHaveLength(8);
  });
});

describe('validateVarName', () => {
  it('接受常规变量名', () => {
    expect(validateVarName('JAVA_HOME')).toBeNull();
    expect(validateVarName('Path')).toBeNull();
  });

  it('拒绝空名与仅空白', () => {
    expect(validateVarName('')).not.toBeNull();
    expect(validateVarName('   ')).not.toBeNull();
  });

  it('拒绝含等号的名字', () => {
    expect(validateVarName('BAD=NAME')).not.toBeNull();
  });

  it('拒绝含 null 字节的名字', () => {
    expect(validateVarName('BAD\0NAME')).not.toBeNull();
  });
});

describe('displayValue', () => {
  it('敏感且未 reveal 时返回占位符', () => {
    expect(displayValue(meta({ sensitive: true, preview: null }), null)).toBe('••••••••');
  });

  it('敏感但已 reveal 时返回明文', () => {
    expect(displayValue(meta({ sensitive: true, preview: null }), 'real-secret')).toBe(
      'real-secret',
    );
  });

  it('不敏感时返回 preview', () => {
    expect(displayValue(meta(), null)).toBe('C:\\Java');
  });

  it('不敏感但 preview 为 null 时返回空串', () => {
    expect(displayValue(meta({ preview: null }), null)).toBe('');
  });

  it('Unsupported 类型显示类型占位，不显示值', () => {
    expect(displayValue(meta({ kind: 'unsupported', preview: null }), null)).toBe(
      '(不支持的注册表类型)',
    );
  });
});

describe('filterEnvVars', () => {
  const snapshot: EnvVarSnapshot = {
    system: [
      meta({ name: 'windir', hive: 'system' }),
      meta({ name: 'MY_KEY', hive: 'system', sensitive: true, preview: null }),
    ],
    user: [meta({ name: 'JAVA_HOME', hive: 'user' })],
  };

  it('filter=all 返回两个 hive', () => {
    expect(filterEnvVars(snapshot, 'all', '')).toHaveLength(3);
  });

  it('filter=system 只返回系统变量', () => {
    const result = filterEnvVars(snapshot, 'system', '');
    expect(result).toHaveLength(2);
    expect(result.every((m) => m.hive === 'system')).toBe(true);
  });

  it('filter=user 只返回用户变量', () => {
    const result = filterEnvVars(snapshot, 'user', '');
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('JAVA_HOME');
  });

  it('空查询返回副本而非快照本体引用', () => {
    // store 可能对结果做原地排序/增删，若返回本体会污染共享快照
    const systemResult = filterEnvVars(snapshot, 'system', '');
    expect(systemResult).not.toBe(snapshot.system);
    expect(systemResult).toEqual(snapshot.system);

    const userResult = filterEnvVars(snapshot, 'user', '');
    expect(userResult).not.toBe(snapshot.user);
    expect(userResult).toEqual(snapshot.user);

    const allResult = filterEnvVars(snapshot, 'all', '');
    expect(allResult).not.toBe(snapshot.system);
    expect(allResult).not.toBe(snapshot.user);
  });

  it('搜索忽略大小写且只匹配变量名', () => {
    expect(filterEnvVars(snapshot, 'all', 'java')).toHaveLength(1);
    expect(filterEnvVars(snapshot, 'all', 'WINDIR')).toHaveLength(1);
  });

  it('搜索不匹配值内容', () => {
    // preview 为 C:\Java，但按 "C:\\" 搜索不应命中
    expect(filterEnvVars(snapshot, 'all', 'C:\\')).toHaveLength(0);
  });
});
