<script setup lang="ts">
import type { SelectTriggerProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { ChevronDown } from "@lucide/vue"
import { reactiveOmit } from "@vueuse/core"
import { cn } from "@/lib/utils"
import { SelectIcon, SelectTrigger, useForwardProps } from "reka-ui"

const props = withDefaults(
  defineProps<SelectTriggerProps & { class?: HTMLAttributes["class"], size?: "sm" | "default" }>(),
  { size: "default" },
)

const delegatedProps = reactiveOmit(props, "class", "size")
const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <SelectTrigger
    data-slot="select-trigger"
    :data-size="size"
    v-bind="forwardedProps"
    :class="cn(
      `data-[placeholder]:text-ft-text-3 [&_svg:not([class*='text-'])]:text-ft-text-3 border-ft-line bg-ft-panel hover:border-ft-line-strong text-ft-text flex w-fit cursor-pointer items-center justify-between gap-1.5 rounded-md border px-2.5 text-sm whitespace-nowrap transition-colors duration-[80ms] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-7 data-[size=sm]:h-6 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5`,
      props.class,
    )"
  >
    <slot />
    <SelectIcon as-child>
      <ChevronDown class="size-3.5" />
    </SelectIcon>
  </SelectTrigger>
</template>
