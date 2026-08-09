/**
 * ThemeSwitcher 组件测试
 *
 * 纯展示组件:验证渲染三个选项 / 点击触发 onChange / 选中态高亮。
 */
import { describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'

import { ThemeSwitcher } from '@components/ThemeSwitcher'

describe('ThemeSwitcher', () => {
  it('渲染三个主题选项(深色/浅色/折中)', () => {
    render(<ThemeSwitcher value="dark" onChange={() => {}} />)
    expect(screen.getByRole('radio', { name: '深色' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '浅色' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '折中' })).toBeInTheDocument()
  })

  it('当前主题的 radio 标记为 checked', () => {
    render(<ThemeSwitcher value="light" onChange={() => {}} />)
    const lightRadio = screen.getByRole('radio', { name: '浅色' })
    const darkRadio = screen.getByRole('radio', { name: '深色' })
    expect(lightRadio).toHaveAttribute('aria-checked', 'true')
    expect(darkRadio).toHaveAttribute('aria-checked', 'false')
  })

  it('点击选项触发 onChange', () => {
    const onChange = vi.fn()
    render(<ThemeSwitcher value="dark" onChange={onChange} />)
    fireEvent.click(screen.getByRole('radio', { name: '折中' }))
    expect(onChange).toHaveBeenCalledOnce()
    expect(onChange).toHaveBeenCalledWith('mid')
  })

  it('radiogroup 有 aria-label', () => {
    render(<ThemeSwitcher value="dark" onChange={() => {}} />)
    expect(screen.getByRole('radiogroup', { name: '界面主题' })).toBeInTheDocument()
  })
})
