import type { PlatformPlaylist } from "../types";

interface Props {
  playlists: PlatformPlaylist[];
  activeKey?: string | null;
  onOpen: (playlist: PlatformPlaylist) => void;
  onToggleSave?: (playlist: PlatformPlaylist) => void;
  isSaved?: (playlist: PlatformPlaylist) => boolean;
  empty?: string;
}

/// 歌单卡片网格：热门歌单与我的收藏共用。
export function playlistKey(playlist: PlatformPlaylist): string {
  return `${playlist.channel}-${playlist.id}`;
}

export default function PlaylistGrid({
  playlists,
  activeKey,
  onOpen,
  onToggleSave,
  isSaved,
  empty,
}: Props) {
  if (playlists.length === 0) {
    return <p className="hint">{empty ?? "还没有歌单。"}</p>;
  }

  return (
    <div className="playlist-grid">
      {playlists.map((playlist) => {
        const key = playlistKey(playlist);
        return (
          <div
            key={key}
            className={key === activeKey ? "playlist-card active" : "playlist-card"}
            onClick={() => onOpen(playlist)}
          >
            <div className="playlist-cover">
              {playlist.artwork_uri ? <img src={playlist.artwork_uri} alt="" /> : null}
            </div>
            <div className="playlist-name" title={playlist.name}>
              {playlist.name}
            </div>
            <div className="playlist-sub">
              {playlist.author || "未知作者"}
              {playlist.track_count ? ` · ${playlist.track_count} 首` : ""}
            </div>
            {onToggleSave ? (
              <button
                className="save"
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleSave(playlist);
                }}
              >
                {isSaved?.(playlist) ? "已收藏" : "收藏"}
              </button>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
