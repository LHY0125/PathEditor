import type { ButtonHTMLAttributes, Ref } from 'react';
import { buttonVariants, type ButtonSize, type ButtonVariant } from './buttons';

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  // React 19：ref 可作为普通 prop 直接透传给 DOM，无需 forwardRef
  ref?: Ref<HTMLButtonElement>;
};

/** 统一按钮：所有变体共享几何，颜色由 globals.css 的 .btn-* 类按主题提供。 */
export function Button({
  variant = 'secondary',
  size = 'md',
  className,
  ref,
  ...rest
}: ButtonProps) {
  const cls = buttonVariants(variant, size);
  return (
    <button ref={ref} type="button" className={className ? `${cls} ${className}` : cls} {...rest} />
  );
}
