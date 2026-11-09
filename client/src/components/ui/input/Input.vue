<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { useVModel } from "@vueuse/core"
import { cn } from "@/lib/utils"

const props = defineProps<{
  defaultValue?: string | number
  modelValue?: string | number
  class?: HTMLAttributes["class"]
}>()

const emits = defineEmits<{
  (e: "update:modelValue", payload: string | number): void
}>()

const modelValue = useVModel(props, "modelValue", emits, {
  passive: true,
  defaultValue: props.defaultValue,
})
</script>

<template>
  <input
    v-model="modelValue"
    data-slot="input"
    :class="cn(
      'placeholder:text-ft-text-3 bg-ft-panel border-ft-line hover:border-ft-line-strong text-ft-text h-7 w-full min-w-0 rounded-md border px-2.5 text-sm transition-colors duration-[80ms] outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50',
      'focus:border-ft-focus focus-visible:outline-none',
      'aria-invalid:border-ft-danger',
      props.class,
    )"
  >
</template>
