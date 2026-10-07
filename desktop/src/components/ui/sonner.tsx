import { CheckCircleIcon, InfoIcon, SpinnerIcon, WarningIcon, XCircleIcon } from "@phosphor-icons/react"

import { Toaster as Sonner, type ToasterProps } from "sonner"

import { useSettings } from "@/settings-context"

const Toaster = ({ ...props }: ToasterProps) => {
  // 跟随应用设置里的深浅主题（shadcn 原版用 next-themes，这里换成自研设置）。
  const { settings } = useSettings()

  return (
    <Sonner
      theme={settings?.dark_theme === false ? "light" : "dark"}
      className="toaster group"
      icons={{
        success: (
          <CheckCircleIcon className="size-4" />
        ),
        info: (
          <InfoIcon className="size-4" />
        ),
        warning: (
          <WarningIcon className="size-4" />
        ),
        error: (
          <XCircleIcon className="size-4" />
        ),
        loading: (
          <SpinnerIcon className="size-4 animate-spin" />
        ),
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "cn-toast",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
