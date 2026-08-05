/**
 * shadcn/ui 标准 className 合并工具
 *
 * cn() 合并 clsx(条件 className) + tailwind-merge(解决 Tailwind 类冲突)。
 * shadcn 组件全部用此 helper 处理 className。
 *
 * 用法:
 *   import { cn } from '@/lib/utils'
 *   <div className={cn('px-2 py-1', isActive && 'bg-primary', className)} />
 */
import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}
