import { MusicNotesIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { loadTrackArtwork } from "@/lib/track-artwork";
import type { Track } from "@/types";

export default function TrackArtwork({ track }: { track: Track }) {
  const container = useRef<HTMLSpanElement>(null);
  const [failed, setFailed] = useState(false);
  const [resolved, setResolved] = useState<string | null>(null);
  const needsLookup = !track.artworkUri?.trim() || failed;

  useEffect(() => {
    if (!needsLookup) return;
    let disposed = false;
    let requested = false;
    const load = () => {
      if (requested) return;
      requested = true;
      void loadTrackArtwork(track, failed).then((url) => {
        if (!disposed) setResolved(url);
      }).catch((error) => console.error("[track artwork]", error));
    };
    if (!container.current || typeof IntersectionObserver === "undefined") {
      load();
      return () => { disposed = true; };
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        observer.disconnect();
        load();
      }
    }, { rootMargin: "160px" });
    observer.observe(container.current);
    return () => { disposed = true; observer.disconnect(); };
  }, [track, needsLookup, failed]);

  const url = resolved ?? (!failed ? track.artworkUri : null);
  return (
    <span ref={container} className="flex size-10 items-center justify-center overflow-hidden rounded-lg bg-muted text-muted-foreground">
      {url ? <img src={url} alt="" loading="lazy" className="size-full object-cover" onError={() => { setFailed(true); setResolved(null); }} /> : <MusicNotesIcon size={18} weight="duotone" />}
    </span>
  );
}
