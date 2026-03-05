<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'

type TaskStatus = 'running' | 'stopped' | 'error'

interface Task {
  id: string
  name: string
  project: string
  workdir: string
  sock_path: string
  ttyd_port: number
  dtach_pid: number | null
  ttyd_pid: number | null
  status: TaskStatus
  created_at: string
  updated_at: string
}

interface AuditLog {
  id: number
  task_id: string | null
  action: string
  detail: string | null
  created_at: string
}

const tasks = ref<Task[]>([])
const logs = ref<AuditLog[]>([])
const loading = ref(false)
const logsLoading = ref(false)
const submitting = ref(false)
const message = ref('')
const error = ref('')
const activeTab = ref<'tasks' | 'create' | 'logs'>('tasks')
const taskFilter = ref<'all' | TaskStatus>('all')
const logFilter = ref<'all' | 'create' | 'start' | 'stop' | 'delete' | 'error'>('all')

const form = reactive({
  project: '',
  name: ''
})

let timer: number | undefined
let flashTimer: number | undefined

const runningCount = computed(() => tasks.value.filter((task) => task.status === 'running').length)
const filteredTasks = computed(() => {
  if (taskFilter.value === 'all') return tasks.value
  return tasks.value.filter((task) => task.status === taskFilter.value)
})
const filteredLogs = computed(() => {
  if (logFilter.value === 'all') return logs.value
  return logs.value.filter((log) => {
    const action = log.action.toLowerCase()
    return action.includes(logFilter.value)
  })
})
const hasToast = computed(() => Boolean(message.value || error.value))

function scheduleFlashClear() {
  if (flashTimer) window.clearTimeout(flashTimer)
  flashTimer = window.setTimeout(() => {
    message.value = ''
    error.value = ''
  }, 3200)
}

function showMessage(text: string) {
  message.value = text
  error.value = ''
  scheduleFlashClear()
}

function showError(text: string) {
  error.value = text
  message.value = ''
  scheduleFlashClear()
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {})
    },
    ...init
  })

  if (!response.ok) {
    let detail = `${response.status} ${response.statusText}`
    try {
      const body = await response.json()
      if (body?.error) detail = body.error
    } catch {
      // ignore parse error
    }
    throw new Error(detail)
  }

  const text = await response.text()
  return (text ? JSON.parse(text) : {}) as T
}

async function loadTasks() {
  loading.value = true
  try {
    tasks.value = await request<Task[]>('/api/tasks')
  } catch (err) {
    showError((err as Error).message)
  } finally {
    loading.value = false
  }
}

async function loadLogs() {
  logsLoading.value = true
  try {
    logs.value = await request<AuditLog[]>('/api/logs?limit=60')
  } catch (err) {
    showError((err as Error).message)
  } finally {
    logsLoading.value = false
  }
}

async function refreshAll() {
  await Promise.all([loadTasks(), loadLogs()])
}

async function createTask() {
  if (!form.project.trim()) {
    showError('请输入 project 名称。')
    activeTab.value = 'create'
    return
  }

  submitting.value = true
  try {
    await request<Task>('/api/tasks', {
      method: 'POST',
      body: JSON.stringify({
        project: form.project.trim(),
        name: form.name.trim() || undefined
      })
    })
    form.project = ''
    form.name = ''
    showMessage('任务已创建。')
    activeTab.value = 'tasks'
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  } finally {
    submitting.value = false
  }
}

async function startTask(task: Task) {
  try {
    await request(`/api/tasks/${task.id}/start`, { method: 'POST' })
    showMessage(`任务 ${task.name} 已启动。`)
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  }
}

async function stopTask(task: Task) {
  try {
    await request(`/api/tasks/${task.id}/stop`, { method: 'POST' })
    showMessage(`任务 ${task.name} 已停止。`)
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  }
}

async function deleteTask(task: Task) {
  if (!window.confirm(`确认删除任务 ${task.name}？`)) return
  try {
    await request(`/api/tasks/${task.id}`, { method: 'DELETE' })
    showMessage(`任务 ${task.name} 已删除。`)
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  }
}

function openTerminal(task: Task) {
  const url = `/term/${task.id}/`
  window.open(url, '_blank', 'noopener,noreferrer')
}

onMounted(async () => {
  await refreshAll()
  timer = window.setInterval(refreshAll, 5000)
})

onUnmounted(() => {
  if (timer) window.clearInterval(timer)
  if (flashTimer) window.clearTimeout(flashTimer)
})
</script>

<template>
  <main class="app">
    <header class="top card">
      <div>
        <p class="eyebrow">RemoteTerminal</p>
        <h1>任务控制台</h1>
        <p class="sub">手机优先视图：任务、创建、日志</p>
      </div>
      <div class="top-meta">
        <span class="stat">总任务 {{ tasks.length }}</span>
        <span class="stat online">运行中 {{ runningCount }}</span>
      </div>
      <button class="ghost small" :disabled="loading || logsLoading" @click="refreshAll">
        {{ loading || logsLoading ? '刷新中...' : '全局刷新' }}
      </button>
    </header>

    <nav class="tabs card">
      <button :class="{ active: activeTab === 'tasks' }" @click="activeTab = 'tasks'">
        任务
        <span class="tab-count">{{ tasks.length }}</span>
      </button>
      <button :class="{ active: activeTab === 'create' }" @click="activeTab = 'create'">创建</button>
      <button :class="{ active: activeTab === 'logs' }" @click="activeTab = 'logs'">
        日志
        <span class="tab-count">{{ logs.length }}</span>
      </button>
    </nav>

    <section v-if="activeTab === 'tasks'" class="panel">
      <div class="filter-row">
        <button class="chip" :class="{ selected: taskFilter === 'all' }" @click="taskFilter = 'all'">全部</button>
        <button class="chip" :class="{ selected: taskFilter === 'running' }" @click="taskFilter = 'running'">运行中</button>
        <button class="chip" :class="{ selected: taskFilter === 'stopped' }" @click="taskFilter = 'stopped'">已停止</button>
        <button class="chip" :class="{ selected: taskFilter === 'error' }" @click="taskFilter = 'error'">错误</button>
      </div>

      <article v-for="task in filteredTasks" :key="task.id" class="card task-card">
        <header class="task-head">
          <strong>{{ task.name }}</strong>
          <span class="badge" :class="task.status">{{ task.status }}</span>
        </header>
        <p class="meta">Project: {{ task.project }}</p>
        <p class="meta">Port: {{ task.ttyd_port }} | Updated: {{ new Date(task.updated_at).toLocaleString() }}</p>
        <p class="muted id-text">{{ task.id }}</p>
        <div class="task-actions">
          <button class="primary block" @click="openTerminal(task)">打开终端</button>
          <button class="ghost" :disabled="task.status === 'running'" @click="startTask(task)">启动</button>
          <button class="ghost" :disabled="task.status !== 'running'" @click="stopTask(task)">停止</button>
          <button class="danger" @click="deleteTask(task)">删除</button>
        </div>
      </article>

      <section v-if="filteredTasks.length === 0" class="card empty">当前筛选下暂无任务。</section>
    </section>

    <section v-if="activeTab === 'create'" class="panel card create-panel">
      <h2>创建任务</h2>
      <label>
        <span>Project</span>
        <input v-model="form.project" placeholder="例如: demo-app" />
      </label>
      <label>
        <span>Name（可选）</span>
        <input v-model="form.name" placeholder="例如: Demo Project" />
      </label>
      <button class="primary block sticky-action" :disabled="submitting" @click="createTask">
        {{ submitting ? '创建中...' : '创建终端任务' }}
      </button>
    </section>

    <section v-if="activeTab === 'logs'" class="panel">
      <div class="filter-row">
        <button class="chip" :class="{ selected: logFilter === 'all' }" @click="logFilter = 'all'">全部</button>
        <button class="chip" :class="{ selected: logFilter === 'create' }" @click="logFilter = 'create'">创建</button>
        <button class="chip" :class="{ selected: logFilter === 'start' }" @click="logFilter = 'start'">启动</button>
        <button class="chip" :class="{ selected: logFilter === 'stop' }" @click="logFilter = 'stop'">停止</button>
        <button class="chip" :class="{ selected: logFilter === 'delete' }" @click="logFilter = 'delete'">删除</button>
        <button class="chip" :class="{ selected: logFilter === 'error' }" @click="logFilter = 'error'">错误</button>
      </div>

      <button class="ghost small refresh-log" :disabled="logsLoading" @click="loadLogs">
        {{ logsLoading ? '刷新中...' : '刷新日志' }}
      </button>

      <article v-for="log in filteredLogs" :key="log.id" class="card log-card">
        <header class="task-head">
          <strong>#{{ log.id }} {{ log.action }}</strong>
          <span class="muted">{{ new Date(log.created_at).toLocaleString() }}</span>
        </header>
        <p class="meta">Task: {{ log.task_id || '-' }}</p>
        <p class="meta detail">{{ log.detail || '-' }}</p>
      </article>

      <section v-if="filteredLogs.length === 0" class="card empty">当前筛选下暂无日志。</section>
    </section>

    <section v-if="hasToast" class="toast" :class="{ err: Boolean(error), ok: Boolean(message) }">
      {{ error || message }}
    </section>
  </main>
</template>
