import { MusicNotesIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

export default function EmptyState({ title, description, children }: {
  title: string;
  description?: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-44 flex-col items-center justify-center rounded-2xl bg-muted/40 px-6 py-8 text-center">
      <span className="mb-3 flex size-11 items-center justify-center rounded-2xl bg-card text-primary">
        <MusicNotesIcon size={22} weight="duotone" />
      </span>
      <p className="text-sm font-medium">{title}</p>
      {description ? <p className="mt-2 max-w-sm text-xs leading-relaxed text-muted-foreground">{description}</p> : null}
      {children ? <div className="mt-4">{children}</div> : null}
    </div>
  );
}
