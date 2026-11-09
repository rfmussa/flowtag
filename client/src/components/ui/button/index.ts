import type { VariantProps } from "class-variance-authority"
import { cva } from "class-variance-authority"

export { default as Button } from "./Button.vue"

/**
 * Retuned to FlowTag's design system (shadcn components are owned code):
 * 28px controls, 6px radius, 12.5px text, 80ms color-only transitions,
 * focus drawn by FlowTag's global :focus-visible outline (flowtag.scss).
 *   ghost       FlowTag's quiet button: secondary text, faint fill on hover
 *   default     the raised primary: raised surface + hairline border
 *   destructive red text, used for deletes in menus and confirms
 */
export const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-md text-sm font-medium transition-colors duration-[80ms] ease-out cursor-pointer disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-[15px] shrink-0 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-ft-raised text-ft-text border border-ft-line-strong hover:bg-ft-overlay",
        destructive:
          "text-ft-danger hover:bg-ft-hover",
        outline:
          "border border-ft-line bg-ft-panel text-ft-text hover:border-ft-line-strong",
        secondary:
          "bg-ft-raised text-ft-text hover:bg-ft-overlay",
        ghost:
          "text-ft-text-2 hover:bg-ft-hover hover:text-ft-text active:bg-ft-active",
        link: "text-ft-text underline-offset-4 hover:underline",
      },
      size: {
        "default": "h-7 px-2.5",
        "xs": "h-5 gap-1 px-1.5 text-xs [&_svg:not([class*='size-'])]:size-3",
        "sm": "h-6 gap-1 px-2 text-xs",
        "lg": "h-8 px-3",
        "icon": "size-7",
        "icon-xs": "size-5 [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-6",
        "icon-lg": "size-8",
      },
    },
    defaultVariants: {
      variant: "ghost",
      size: "default",
    },
  },
)
export type ButtonVariants = VariantProps<typeof buttonVariants>
