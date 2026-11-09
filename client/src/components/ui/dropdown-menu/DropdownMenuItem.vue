<script setup lang="ts">
import type { DropdownMenuItemProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { cn } from "@/lib/utils"
import { DropdownMenuItem, useForwardProps } from "reka-ui"

const props = withDefaults(defineProps<DropdownMenuItemProps & {
  class?: HTMLAttributes["class"]
  inset?: boolean
  variant?: "default" | "destructive"
}>(), {
  variant: "default",
})

const delegatedProps = reactiveOmit(props, "inset", "variant", "class")

const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <DropdownMenuItem
    data-slot="dropdown-menu-item"
    :data-inset="inset ? '' : undefined"
    :data-variant="variant"
    v-bind="forwardedProps"
    :class="cn(`relative flex cursor-default items-center gap-2 rounded-md px-2 h-7 text-sm text-ft-text-2 outline-hidden select-none focus:bg-ft-hover focus:text-ft-text data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8 data-[variant=destructive]:text-ft-danger data-[variant=destructive]:focus:text-ft-danger [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5 [&_svg:not([class*='text-'])]:text-ft-text-3 data-[variant=destructive]:*:[svg]:text-ft-danger!`, props.class)"
  >
    <slot />
  </DropdownMenuItem>
</template>
