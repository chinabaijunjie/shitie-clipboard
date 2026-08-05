/**
 * HomePage — 示例页面（演示前端调后端全链路）
 *
 * 调用 /api/items CRUD 五端点，演示：
 * - apiClient 拆包 ApiResponse<T>
 * - ApiClientError 错误处理（useNotify 友好提示）
 * - 加载 / 刷新 / 创建 / 删除 交互
 *
 * 业务工程起步时，参照此页面写业务页面（如 ImageListPage）。
 */
import { useCallback, useEffect, useState } from 'react'
import { Button, Card, Input, List, Popconfirm, Space, Typography } from 'antd'
import { DeleteOutlined, PlusOutlined, ReloadOutlined } from '@ant-design/icons'

import { apiClient, ApiClientError } from '@core/api'
import { useNotify } from '@components/ui'
import type { Item } from '@core/types'

const { Title, Text } = Typography

export function HomePage() {
  const notify = useNotify()
  const [items, setItems] = useState<Item[]>([])
  const [loading, setLoading] = useState(false)
  const [newName, setNewName] = useState('')

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const data = await apiClient.get<Item[]>('/api/items')
      setItems(data)
    } catch (err) {
      notify.error(err)
    } finally {
      setLoading(false)
    }
  }, [notify])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const handleCreate = async () => {
    const name = newName.trim()
    if (!name) {
      notify.warning('请输入名称')
      return
    }
    try {
      await apiClient.post<Item>('/api/items', { name })
      setNewName('')
      notify.success('创建成功')
      void refresh()
    } catch (err) {
      notify.error(err)
    }
  }

  const handleDelete = async (id: string) => {
    try {
      await apiClient.delete(`/api/items/${id}`)
      notify.success('删除成功')
      void refresh()
    } catch (err) {
      notify.error(err)
    }
  }

  return (
    <div style={{ padding: 24, maxWidth: 800, margin: '0 auto' }}>
      <Space style={{ marginBottom: 16, width: '100%', justifyContent: 'space-between' }}>
        <Title level={3} style={{ margin: 0 }}>
          Items（示例 CRUD）
        </Title>
        <Button icon={<ReloadOutlined />} onClick={() => void refresh()} loading={loading}>
          刷新
        </Button>
      </Space>

      <Card style={{ marginBottom: 16 }}>
        <Space.Compact style={{ width: '100%' }}>
          <Input
            placeholder="输入名称创建新 item"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onPressEnter={() => void handleCreate()}
            maxLength={255}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={() => void handleCreate()}>
            创建
          </Button>
        </Space.Compact>
      </Card>

      <Card loading={loading}>
        {items.length === 0 && !loading ? (
          <Text type="secondary">暂无 items，创建第一个吧</Text>
        ) : (
          <List
            dataSource={items}
            rowKey="id"
            renderItem={(item) => (
              <List.Item
                actions={[
                  <Popconfirm
                    key="delete"
                    title="确认删除？"
                    onConfirm={() => void handleDelete(item.id)}
                  >
                    <Button danger size="small" icon={<DeleteOutlined />}>
                      删除
                    </Button>
                  </Popconfirm>,
                ]}
              >
                <List.Item.Meta
                  title={item.name}
                  description={item.description ?? <Text type="secondary">无描述</Text>}
                />
              </List.Item>
            )}
          />
        )}
      </Card>

      <ErrorHint />
    </div>
  )
}

/**
 * 错误码契约演示：如果后端返回 ApiClientError，展示其 code/message。
 * 业务工程可移除此提示，仅作模板演示用。
 */
function ErrorHint() {
  return (
    <Text
      type="secondary"
      style={{ display: 'block', marginTop: 24, fontSize: 12 }}
    >
      提示：此页面演示前端调后端全链路。后端未启动时 API 请求会失败并提示
      「{ApiClientError.name}」错误码 SIDECAR_NOT_READY。
    </Text>
  )
}
