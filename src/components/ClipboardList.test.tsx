/**
 * ClipboardList 组件测试
 *
 * 阶段A(预览面板规划):行高统一回归 —— 选中不展开三行 / 图片不放大,
 * 完整内容由右侧预览面板展示(见 wiki/dev/preview-panel-plan.md)。
 */
import { describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'

// convertFileSrc 依赖 Tauri 运行时,测试环境 mock 掉(图片条目用)
vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://mock${path}`,
}))

import { ClipboardList, previewText } from '@components/ClipboardList'
import type { ClipboardItem } from '@core/types'

/** 最小可用条目 fixture(可选字段按需覆盖) */
function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    firstCopiedAt: 1728800000000,
    lastCopiedAt: 1728800000000,
    copyCount: 1,
    title: '示例文本',
    contentKind: 'text',
    pinned: false,
    ...overrides,
  }
}

describe('previewText(单行截断纯函数)', () => {
  it('短文本原样返回,不加省略号', () => {
    expect(previewText('短文本')).toBe('短文本')
  })

  it('超长文本截断到 60 字符并加省略号', () => {
    const long = 'a'.repeat(100)
    const result = previewText(long)
    expect(result).toBe('a'.repeat(60) + '…')
  })

  it('恰好 60 字符不加省略号', () => {
    const exact = 'b'.repeat(60)
    expect(previewText(exact)).toBe(exact)
  })

  it('空文本返回占位符', () => {
    expect(previewText('')).toBe('<空>')
  })
})

describe('ClipboardList 行高统一(选中不展开)', () => {
  it('选中态文字仍单行截断(无 line-clamp-3 / whitespace-normal)', () => {
    const long = 'x'.repeat(100)
    render(
      <ClipboardList
        items={[makeItem({ title: long })]}
        selectedIndex={0}
      />,
    )
    const span = screen.getByText('x'.repeat(60) + '…')
    expect(span.className).toContain('whitespace-nowrap')
    expect(span.className).not.toContain('line-clamp-3')
    expect(span.className).not.toContain('whitespace-normal')
  })

  it('选中态仍高亮(仅变色,li 带 text-primary)', () => {
    render(
      <ClipboardList items={[makeItem()]} selectedIndex={0} />,
    )
    const li = screen.getByRole('listitem')
    expect(li.className).toContain('text-primary')
  })

  it('图片缩略图选中时仍为 20px(不放大到 h-10)', () => {
    const { container } = render(
      <ClipboardList
        items={[makeItem({
          contentKind: 'image',
          title: '图片 800×600',
          thumbPath: '/images/thumb/abc.png',
          imagePath: '/images/orig/abc.png',
        })]}
        selectedIndex={0}
      />,
    )
    // alt="" 的 img 无隐式 role(装饰图),直接 DOM 查询
    const img = container.querySelector('img')
    expect(img).not.toBeNull()
    expect(img!.className).toContain('h-5')
    expect(img!.className).toContain('w-5')
    expect(img!.className).not.toContain('h-10')
    expect(img).toHaveAttribute('src', 'asset://mock/images/thumb/abc.png')
  })

  it('空列表渲染空态提示', () => {
    render(<ClipboardList items={[]} selectedIndex={-1} />)
    expect(screen.getByText('还没有复制记录')).toBeInTheDocument()
  })

  it('单击选中 / 双击回填回调触发', () => {
    const onSelect = vi.fn()
    const onCopyBack = vi.fn()
    render(
      <ClipboardList
        items={[makeItem()]}
        selectedIndex={0}
        onSelect={onSelect}
        onCopyBack={onCopyBack}
      />,
    )
    const li = screen.getByRole('listitem')
    fireEvent.click(li)
    expect(onSelect).toHaveBeenCalledWith(0)
    fireEvent.doubleClick(li)
    expect(onCopyBack).toHaveBeenCalledWith(0)
  })
})
