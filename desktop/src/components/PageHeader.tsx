import type { ReactNode } from "react";

export default function PageHeader({ title, description, eyebrow, children }: {
  title: string;
  description?: string;
  eyebrow?: string;
  children?: ReactNode;
}) {
  return (
    <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        {eyebrow ? (
          <div className="mb-2 flex items-center gap-2 text-[10px] font-medium tracking-[0.22em] text-primary/85 uppercase">
            <span className="size-1 rounded-full bg-primary" />
            {eyebrow}
          </div>
        ) : null}
        <h1 className="text-[30px] leading-[1.12] font-semibold tracking-[-0.035em]">{title}</h1>
        {description ? <p className="mt-2 max-w-[62ch] text-[13px] leading-relaxed text-muted-foreground">{description}</p> : null}
      </div>
      {children ? <div className="flex shrink-0 items-center gap-2">{children}</div> : null}
    </header>
  );
}
