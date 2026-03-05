import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from './App.vue'

describe('App', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('renders dashboard title and loads tasks and logs', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url === '/api/tasks') {
        return new Response('[]', {
          status: 200,
          headers: { 'Content-Type': 'application/json' }
        })
      }
      if (url === '/api/logs?limit=60') {
        return new Response('[]', {
          status: 200,
          headers: { 'Content-Type': 'application/json' }
        })
      }
      return new Response('{}', {
        status: 200,
        headers: { 'Content-Type': 'application/json' }
      })
    })

    vi.stubGlobal('fetch', fetchMock)

    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.text()).toContain('多目录并行任务控制台')
    expect(fetchMock).toHaveBeenCalledWith('/api/tasks', expect.anything())
    expect(fetchMock).toHaveBeenCalledWith('/api/logs?limit=60', expect.anything())
  })
})
