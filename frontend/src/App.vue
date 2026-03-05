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

const form = reactive({
  project: '',
  name: ''
})

let timer: number | undefined

const runningCount = computed(() => tasks.value.filter((task) => task.status === 'running').length)

function showMessage(text: string) {
  message.value = text
  error.value = ''
}

function showError(text: string) {
  error.value = text
  message.value = ''
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
})
</script>

<template>
  <main class="page">
    <section class="hero card">
      <div>
        <p class="eyebrow">RemoteTerminal</p>
        <h1>RemoteTerminal 多目录并行任务控制台</h1>
        <p class="sub">创建目录任务、后台运行 Codex、通过 /term/{{ '{task_id}' }} 回连。</p>
      </div>
      <div class="stats">
        <span class="stat">总任务 {{ tasks.length }}</span>
        <span class="stat online">运行中 {{ runningCount }}</span>
      </div>
    </section>

    <section class="card form-card">
      <h2>创建任务</h2>
      <div class="grid">
        <label>
          <span>Project</span>
          <input v-model="form.project" placeholder="例如: demo-app" />
        </label>
        <label>
          <span>Name（可选）</span>
          <input v-model="form.name" placeholder="例如: Demo Project" />
        </label>
      </div>
      <button class="primary" :disabled="submitting" @click="createTask">{{ submitting ? '创建中...' : '创建终端任务' }}</button>
      <p v-if="message" class="msg ok">{{ message }}</p>
      <p v-if="error" class="msg err">{{ error }}</p>
    </section>

    <section class="card">
      <header class="section-head">
        <h2>任务列表</h2>
        <button class="ghost" :disabled="loading" @click="refreshAll">{{ loading ? '刷新中...' : '刷新' }}</button>
      </header>

      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Project</th>
              <th>Status</th>
              <th>Port</th>
              <th>Updated</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="task in tasks" :key="task.id">
              <td>
                <strong>{{ task.name }}</strong>
                <p class="muted">{{ task.id }}</p>
              </td>
              <td>{{ task.project }}</td>
              <td>
                <span class="badge" :class="task.status">{{ task.status }}</span>
              </td>
              <td>{{ task.ttyd_port }}</td>
              <td>{{ new Date(task.updated_at).toLocaleString() }}</td>
              <td>
                <div class="actions">
                  <button class="ghost" @click="openTerminal(task)">打开终端</button>
                  <button class="ghost" :disabled="task.status === 'running'" @click="startTask(task)">启动</button>
                  <button class="ghost" :disabled="task.status !== 'running'" @click="stopTask(task)">停止</button>
                  <button class="danger" @click="deleteTask(task)">删除</button>
                </div>
              </td>
            </tr>
            <tr v-if="tasks.length === 0">
              <td colspan="6" class="empty">暂无任务，先创建一个目录任务。</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="card">
      <header class="section-head">
        <h2>最近日志</h2>
        <button class="ghost" :disabled="logsLoading" @click="loadLogs">{{ logsLoading ? '刷新中...' : '刷新日志' }}</button>
      </header>

      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>Task</th>
              <th>Action</th>
              <th>Detail</th>
              <th>Time</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="log in logs" :key="log.id">
              <td>{{ log.id }}</td>
              <td>{{ log.task_id || '-' }}</td>
              <td>{{ log.action }}</td>
              <td>{{ log.detail || '-' }}</td>
              <td>{{ new Date(log.created_at).toLocaleString() }}</td>
            </tr>
            <tr v-if="logs.length === 0">
              <td colspan="5" class="empty">暂无日志。</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </main>
</template>
