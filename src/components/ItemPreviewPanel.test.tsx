/**
 * ItemPreviewPanel 组件测试
 *
 * 纯展示组件:验证四类条目渲染(文字/图片/文件/链接)+ 元信息字段 + 空态。
 * convertFileSrc 依赖 Tauri 运行时,mock 掉。
 */
import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://mock${path}`,
}))

import { ItemPreviewPanel, formatDateTime } from '@components/ItemPreviewPanel'
import type { ClipboardItem } from '@core/types'

function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    firstCopiedAt: new Date('2026-10-13T09:05:03').getTime(),
    lastCopiedAt: new Date('2026-10-13T21:07:00').getTime(),
    copyCount: 3,
    title: '示例文本',
    contentKind: 'text',
    pinned: false,
    ...overrides,
  }
}

describe('formatDateTime(时间格式化纯函数)', () => {
  it('格式化为 YYYY-MM-DD HH:mm:ss,个位数补零', () => {
    expect(formatDateTime(new Date('2026-01-05T08:09:01').getTime())).toBe('2026-01-05 08:09:01')
  })
})

describe('ItemPreviewPanel', () => {
  it('无选中(null)→ 渲染空态提示', () => {
    render(<ItemPreviewPanel item={null} fullText="" />)
    expect(screen.getByText('选择一条记录')).toBeInTheDocument()
  })

  it('文字条目:显示全文 + 元信息(类型/时间/次数/未置顶)', () => {
    render(<ItemPreviewPanel item={makeItem()} fullText="这是完整内容" />)
    expect(screen.getByText('这是完整内容')).toBeInTheDocument()
    expect(screen.getByText('文字')).toBeInTheDocument()
    expect(screen.getByText('2026-10-13 09:05:03')).toBeInTheDocument()
    expect(screen.getByText('2026-10-13 21:07:00')).toBeInTheDocument()
    expect(screen.getByText('3 次')).toBeInTheDocument()
    expect(screen.getByText('未置顶')).toBeInTheDocument()
  })

  it('fullText 为空字符串 → 显示占位符 <空>', () => {
    render(<ItemPreviewPanel item={makeItem({ title: '' })} fullText="" />)
    expect(screen.getByText('<空>')).toBeInTheDocument()
  })

  it('图片条目:渲染原图(优先 imagePath)+ 类型「图片」+ 路径', () => {
    const { container } = render(
      <ItemPreviewPanel
        item={makeItem({
          contentKind: 'image',
          title: '图片 800×600',
          thumbPath: '/images/thumb/abc.png',
          imagePath: '/images/orig/abc.png',
        })}
        fullText=""
      />,
    )
    const img = container.querySelector('img')
    expect(img).not.toBeNull()
    expect(img).toHaveAttribute('src', 'asset://mock/images/orig/abc.png')
    expect(screen.getByText('图片')).toBeInTheDocument()
    expect(screen.getByText('/images/orig/abc.png')).toBeInTheDocument()
  })

  it('文件条目:显示文件名 + 类型「文件」+ 文件路径', () => {
    render(
      <ItemPreviewPanel
        item={makeItem({ contentKind: 'file', title: 'report.pdf', fileUrl: 'file:///Users/x/report.pdf' })}
        fullText=""
      />,
    )
    expect(screen.getByText('report.pdf')).toBeInTheDocument()
    expect(screen.getByText('文件')).toBeInTheDocument()
    expect(screen.getByText('file:///Users/x/report.pdf')).toBeInTheDocument()
  })

  it('链接子类型:类型显示「链接」', () => {
    render(
      <ItemPreviewPanel
        item={makeItem({ contentSubtype: 'link', title: 'https://example.com' })}
        fullText="https://example.com"
      />,
    )
    expect(screen.getByText('链接')).toBeInTheDocument()
  })

  it('置顶条目:显示置顶时间', () => {
    render(
      <ItemPreviewPanel
        item={makeItem({ pinned: true, pinnedAt: new Date('2026-10-12T10:00:00').getTime() })}
        fullText="x"
      />,
    )
    expect(screen.getByText('置顶于 2026-10-12 10:00:00')).toBeInTheDocument()
  })
})
