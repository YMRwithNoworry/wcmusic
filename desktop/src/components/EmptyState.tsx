import { MusicNotesIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

export default function EmptyState({ title, description, children }: {
  title: string;
  description?: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-44 flex-col items-center justify-center rounded-2xl border border-border/60 bg-gradient-to-b from-muted/45 to-muted/15 px-6 py-8 text-center">
      <span className="mb-3 flex size-11 items-center justify-center rounded-2xl bg-card text-primary shadow-sm ring-1 ring-border">
        <MusicNotesIcon size={22} weight="duotone" />
      </span>
      <p className="text-sm font-medium">{title}</p>
      {description ? <p className="mt-2 max-w-sm text-xs leading-relaxed text-muted-foreground">{description}</p> : null}
      {children ? <div className="mt-4">{children}</div> : null}
    </div>
  );
}
