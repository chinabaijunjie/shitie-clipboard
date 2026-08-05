/**
 * invoke.ts 单测 — Tauri invoke 通信层封装
 *
 * 测试覆盖:
 * - call 成功:返回 Rust 侧数据
 * - call 失败(Rust Err):catch 成 InvokeError,保留 code/message
 * - call 失败(非预期格式):归为 UNKNOWN
 *
 * mock 策略:vi.mock '@tauri-apps/api/core' 的 invoke 函数。
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call, InvokeError, InvokeErrorCode } from './invoke'

// mock @tauri-apps/api/core 的 invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const { invoke } = await import('@tauri-apps/api/core')

describe('call (invoke 封装)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('Rust 返回数据 → 原样返回', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('ok')

    const result = await call<string>('ping')

    expect(result).toBe('ok')
    expect(invoke).toHaveBeenCalledWith('ping', undefined)
  })

  it('带 args 调用 → 透传给 invoke', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: 1, name: 'test' })

    const result = await call<{ id: number; name: string }>('get_item', {
      id: 1,
    })

    expect(result).toEqual({ id: 1, name: 'test' })
    expect(invoke).toHaveBeenCalledWith('get_item', { id: 1 })
  })

  it('Rust 返回 Err({code,message}) → 抛 InvokeError 保留 code', async () => {
    vi.mocked(invoke).mockRejectedValueOnce({
      code: 'INTERNAL_ERROR',
      message: 'DB 连接失败',
    })

    await expect(call('get_item')).rejects.toMatchObject({
      name: 'InvokeError',
      code: 'INTERNAL_ERROR',
      message: 'DB 连接失败',
    })
  })

  it('reject 非预期格式(无 code/message)→ 归为 UNKNOWN', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('some string error')

    await expect(call('get_item')).rejects.toMatchObject({
      name: 'InvokeError',
      code: InvokeErrorCode.UNKNOWN,
    })
  })

  it('reject 是 Error 实例 → message 用 Error.message', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('serialization failed'))

    await expect(call('get_item')).rejects.toMatchObject({
      code: InvokeErrorCode.UNKNOWN,
      message: 'serialization failed',
    })
  })
})

describe('InvokeError', () => {
  it('已知 code → 保留原 code', () => {
    const err = new InvokeError('INTERNAL_ERROR', 'boom')
    expect(err.code).toBe('INTERNAL_ERROR')
    expect(err.message).toBe('boom')
    expect(err.name).toBe('InvokeError')
  })

  it('未知 code(两端失同步)→ 归为 UNKNOWN', () => {
    const err = new InvokeError('SOME_NEW_CODE_NOT_IN_ENUM', 'boom')
    expect(err.code).toBe(InvokeErrorCode.UNKNOWN)
  })
})
