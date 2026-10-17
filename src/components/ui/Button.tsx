import type { ButtonHTMLAttributes } from 'react';
import { buttonVariants, type ButtonSize, type ButtonVariant } from './buttons';

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

/** 统一按钮：所有变体共享几何，颜色由 globals.css 的 .btn-* 类按主题提供。 */
export function Button({ variant = 'secondary', size = 'md', className, ...rest }: ButtonProps) {
  const cls = buttonVariants(variant, size);
  return <button type="button" className={className ? `${cls} ${className}` : cls} {...rest} />;
}
