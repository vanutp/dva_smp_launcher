<script setup lang="ts">
interface Props {
  username: string
  memory: number
  javaPath: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'update:memory', v: number): void
  (e: 'update:javaPath', v: string): void
  (e: 'start'): void
}>()

function memoryChanged(event: Event) {
  const el = event.target as HTMLInputElement
  const val = Number(el.value)
  if (val) {
    emit('update:memory', val)
  } else if (el.value != '') {
    el.value = props.memory.toString()
  }
}
</script>

<template>
  <div style="display: flex; flex-direction: column">
    <p>Вы вошли как {{ props.username }}</p>
    <label>Выделенная память</label>
    <input
      :value="props.memory"
      @input="memoryChanged"
    >
    <label>Путь к jabe</label>
    <input
      :value="props.javaPath"
      @input="emit('update:javaPath', $event.target.value)"
    >
    <button @click="emit('start')">
      Поехали!
    </button>
  </div>
</template>

<style scoped>

</style>
