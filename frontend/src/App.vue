<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'

type TaskStatus = 'running' | 'stopped' | 'error'

interface Task {
  id: string
  name: string
  project: string
  workdir: string
  sock_path: string
  ttyd_port: number | null
  dtach_pid: number | null
  ttyd_pid: number | null
  dtach_command: string
  ttyd_command: string | null
  status: TaskStatus
  session_started_at: string | null
  terminal_started_at: string | null
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
const taskActionId = ref<string | null>(null)
const message = ref('')
const error = ref('')
const activeTab = ref<'tasks' | 'create' | 'logs'>('tasks')
const taskFilter = ref<'all' | TaskStatus>('running')
const logFilter = ref<'all' | 'create' | 'start' | 'stop' | 'delete' | 'error'>('all')
const taskFilterTouched = ref(false)
const taskFilterInitialized = ref(false)

const form = reactive({
  project: ''
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

function isTaskBusy(task: Task) {
  return taskActionId.value === task.id
}

function taskActionLabel(task: Task) {
  if (isTaskBusy(task)) {
    return task.status === 'running' ? '停止中...' : '启动中...'
  }
  return task.status === 'running' ? '停止' : '启动'
}

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
    const nextTasks = await request<Task[]>('/api/tasks')
    tasks.value = nextTasks

    if (!taskFilterInitialized.value && !taskFilterTouched.value) {
      const hasRunning = nextTasks.some((task) => task.status === 'running')
      taskFilter.value = hasRunning ? 'running' : 'all'
      taskFilterInitialized.value = true
    }
  } catch (err) {
    showError((err as Error).message)
  } finally {
    loading.value = false
  }
}

function setTaskFilter(filter: 'all' | TaskStatus) {
  taskFilterTouched.value = true
  taskFilter.value = filter
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
        project: form.project.trim()
      })
    })
    form.project = ''
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
  taskActionId.value = task.id
  try {
    await request(`/api/tasks/${task.id}/start`, { method: 'POST' })
    showMessage(`任务 ${task.name} 已启动。`)
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  } finally {
    taskActionId.value = null
  }
}

async function stopTask(task: Task) {
  taskActionId.value = task.id
  try {
    await request(`/api/tasks/${task.id}/stop`, { method: 'POST' })
    showMessage(`任务 ${task.name} 已停止。`)
    await refreshAll()
  } catch (err) {
    showError((err as Error).message)
  } finally {
    taskActionId.value = null
  }
}

function openTerminal(task: Task) {
  const url = `/term/${task.id}/`
  window.open(url, '_blank', 'noopener,noreferrer')
}

function formatTime(value: string | null) {
  if (!value) return '-'
  return new Date(value).toLocaleString()
}

function formatRuntimeTimes(task: Task) {
  return `dtach ${formatTime(task.session_started_at)} | ttyd ${formatTime(task.terminal_started_at)}`
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
        <button class="chip" :class="{ selected: taskFilter === 'all' }" @click="setTaskFilter('all')">全部</button>
        <button class="chip" :class="{ selected: taskFilter === 'running' }" @click="setTaskFilter('running')">运行中</button>
        <button class="chip" :class="{ selected: taskFilter === 'stopped' }" @click="setTaskFilter('stopped')">已停止</button>
        <button class="chip" :class="{ selected: taskFilter === 'error' }" @click="setTaskFilter('error')">错误</button>
      </div>

      <article v-for="task in filteredTasks" :key="task.id" class="card task-card">
        <header class="task-head">
          <strong>{{ task.name }}</strong>
          <div class="task-head-controls">
            <span class="badge" :class="task.status">{{ task.status }}</span>
            <button
              class="ghost control-button"
              :disabled="isTaskBusy(task)"
              @click="task.status === 'running' ? stopTask(task) : startTask(task)"
            >
              {{ taskActionLabel(task) }}
            </button>
            <button
              class="primary control-button"
              :disabled="task.status !== 'running' || isTaskBusy(task)"
              @click="openTerminal(task)"
            >
              打开终端
            </button>
          </div>
        </header>
        <p class="meta">{{ formatRuntimeTimes(task) }}</p>
        <p class="muted id-text">{{ task.dtach_command }}</p>
        <p v-if="task.ttyd_command" class="muted id-text">{{ task.ttyd_command }}</p>
      </article>

      <section v-if="filteredTasks.length === 0" class="card empty">当前筛选下暂无任务。</section>
    </section>

    <section v-if="activeTab === 'create'" class="panel card create-panel">
      <h2>创建任务</h2>
      <label>
        <span>Project</span>
        <input v-model="form.project" placeholder="例如: demo-app" />
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
