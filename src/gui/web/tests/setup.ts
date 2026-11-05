/**
 * Vitest 全局 setup。
 *
 * - 注册 jest-dom 匹配器；
 * - 注册 RTL 的**手动 cleanup**：本仓库没有开 `globals`，`@testing-library/react`
 *   依赖全局 `afterEach` 的自动清理不会生效。不注册时上一个用例的组件仍挂载，
 *   会对后续用例产生跨用例污染（`use-path-validation` 的「期望 3 实得 11」）。
 */
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  cleanup();
});
