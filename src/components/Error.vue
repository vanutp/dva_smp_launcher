<script setup lang="ts">
interface Props {
  message: string
}
const props = defineProps<Props>()
</script>

<template>
  Error: {{ props.message }}
</template>

<style scoped>

</style>
