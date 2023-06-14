<script setup lang="ts">
interface Props {
  progress: number | null
  message: string
}
const props = defineProps<Props>()
</script>

<template>
  Loading... {{ props.message }}
  {{ props.progress}}
</template>

<style scoped>

</style>
