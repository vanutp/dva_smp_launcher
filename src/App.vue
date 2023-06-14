<script setup lang="ts">
import Error from './components/Error.vue'
import Loading from './components/Loading.vue'
import { emit, listen } from '@tauri-apps/api/event'
import { onBeforeUnmount, ref } from 'vue'
import Config from './components/Config.vue'

enum Page {
  Loading,
  Error,
  Config,
}

const page = ref(Page.Loading)

const errorMessage = ref('')

const loadingProgress = ref<number | null>(null)
const loadingMessage = ref('')

const configUsername = ref('')
const configMemory = ref(-1874)
const configJavaPath = ref('')

interface SetError {
  message: string
}
interface SetProgress {
  message: string
  progress: number | null
}
interface SetConfig {
  username: string
  memory: number
  java_path: string
}

const ul1 = await listen<SetError>('set_error', (event) => {
  page.value = Page.Error
  errorMessage.value = event.payload.message
})
const ul2 = await listen<SetProgress>('set_progress', (event) => {
  page.value = Page.Loading
  loadingMessage.value = event.payload.message
  loadingProgress.value = event.payload.progress
})
const ul3 = await listen<SetConfig>('set_config', (event) => {
  page.value = Page.Config
  configUsername.value = event.payload.username
  configMemory.value = event.payload.memory
  configJavaPath.value = event.payload.java_path
})
function start() {
  emit('start', {
    memory: configMemory.value,
    java_path: configJavaPath.value,
  })
}
onBeforeUnmount(() => {
  ul1()
  ul2()
  ul3()
})
</script>

<template>
  <Loading v-if="page == Page.Loading" :progress="loadingProgress" :message="loadingMessage"/>
  <Error v-if="page == Page.Error" :message="errorMessage"/>
  <Config
    v-if="page == Page.Config"
    :username="configUsername"
    v-model:memory="configMemory"
    v-model:java-path="configJavaPath"
    @start="start"
  />
</template>
