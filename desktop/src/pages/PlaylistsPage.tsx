import { FolderPlusIcon } from "@phosphor-icons/react";
import { useEffect, useState, type FormEvent } from "react";

import { favorites, online } from "@/api";
import PageHeader from "@/components/PageHeader";
import PlaylistDetail from "@/components/PlaylistDetail";
import PlaylistGrid, { playlistKey } from "@/components/PlaylistGrid";
import TrackList from "@/components/TrackList";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { favoriteFolderNames } from "@/lib/favorite-folders";
import { useSettings } from "@/settings-context";
import type { PlatformPlaylist, Track } from "@/types";

/// 歌单页：收藏的歌单 + 「爱听的」四个文件夹。
export default function PlaylistsPage() {
  const { settings, replace } = useSettings();
  const [folder, setFolder] = useState("我的收藏");
  const [createOpen, setCreateOpen] = useState(false);
  const [newFolder, setNewFolder] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  /// 点开的歌单：`tracks` 为 `null` 表示还在载入。
  const [detail, setDetail] = useState<{ name: string; tracks: Track[] | null } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const folders = favoriteFolderNames(settings);
  const folderKey = folders.join("\u0000");

  useEffect(() => {
    if (!folders.includes(folder)) {
      setFolder(folders[0] ?? "我的收藏");
    }
  }, [folder, folderKey]);

  const openPlaylist = async (playlist: PlatformPlaylist) => {
    setDetail({ name: playlist.name, tracks: null });
    setError(null);
    try {
      setDetail({ name: playlist.name, tracks: await online.playlistTracks(playlist) });
    } catch (problem) {
      setError(String(problem));
      setDetail(null);
    }
  };

  const savedPlaylists = settings?.saved_playlists ?? [];
  const savedTracks = settings?.saved_tracks ?? [];
  const isSaved = (playlist: PlatformPlaylist) =>
    savedPlaylists.some((item) => playlistKey(item.playlist) === playlistKey(playlist));
  const isTrackSaved = (track: Track, targetFolder = folder) =>
    savedTracks.some(
      (item) =>
        item.folder === targetFolder &&
        item.track.id === track.id &&
        item.track.source === track.source,
    );
  const toggleTrack = (track: Track, targetFolder = folder) => {
    void favorites.toggleTrack(track, targetFolder).then(replace);
  };
  const folderTracks = savedTracks
    .filter((item) => item.folder === folder)
    .map((item) => item.track);

  const openCreateFolder = () => {
    setNewFolder("");
    setCreateError(null);
    setCreateOpen(true);
  };

  const createFolder = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const name = newFolder.trim();
    if (!name) {
      setCreateError("请输入文件夹名称");
      return;
    }
    try {
      const next = await favorites.createFolder(name);
      replace(next);
      setFolder(name);
      setCreateOpen(false);
      setNewFolder("");
    } catch (problem) {
      setCreateError(String(problem));
    }
  };

  if (detail) {
    return (
      <PlaylistDetail
        name={detail.name}
        tracks={detail.tracks}
        onBack={() => setDetail(null)}
        onToggleSave={toggleTrack}
        saved={isTrackSaved}
        folders={folders}
        defaultFolder={folder}
        onCreateFolder={openCreateFolder}
      />
    );
  }

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <PageHeader title="我的收藏" description="收藏的歌单与歌曲都在这里，随时回来接着听。">
        <Button variant="outline" onClick={openCreateFolder}>
          <FolderPlusIcon />新建文件夹
        </Button>
      </PageHeader>

      {error ? (
        <div className="mb-4 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <section className="mb-8 flex flex-col gap-3">
        <h2 className="flex items-center gap-2 text-[16px] font-semibold tracking-tight">
          <span aria-hidden="true" className="h-4 w-[3px] rounded-full bg-gradient-to-b from-primary to-primary/25" />
          收藏的歌单
        </h2>
        <PlaylistGrid
          playlists={savedPlaylists.map((item) => item.playlist)}
          onOpen={(playlist) => void openPlaylist(playlist)}
          onToggleSave={(playlist) => {
            void favorites.togglePlaylist(playlist).then(replace);
          }}
          isSaved={isSaved}
          empty="还没有收藏的歌单。"
        />
      </section>

      <section className="flex flex-col gap-3">
        <h2 className="flex items-center gap-2 text-[16px] font-semibold tracking-tight">
          <span aria-hidden="true" className="h-4 w-[3px] rounded-full bg-gradient-to-b from-primary to-primary/25" />
          收藏的歌曲
        </h2>
        <div className="flex flex-wrap gap-1.5">
          {folders.map((name) => (
            <Button
              key={name}
              size="sm"
              variant={name === folder ? "default" : "outline"}
              aria-pressed={name === folder}
              onClick={() => setFolder(name)}
            >
              {name}
            </Button>
          ))}
          <Button size="sm" variant="outline" onClick={openCreateFolder}>
            <FolderPlusIcon />新建文件夹
          </Button>
        </div>
        <TrackList
          tracks={folderTracks}
          folders={folders}
          defaultFolder={folder}
          onToggleSave={toggleTrack}
          saved={isTrackSaved}
          onCreateFolder={openCreateFolder}
          empty={`「${folder}」还没有歌曲，在榜单或歌单里点行尾的星标加进来。`}
        />
      </section>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>新建收藏文件夹</DialogTitle>
            <DialogDescription>给想要整理的歌曲建立一个新的收藏位置。</DialogDescription>
          </DialogHeader>
          <form className="flex flex-col gap-4" onSubmit={(event) => void createFolder(event)}>
            <Input
              autoFocus
              value={newFolder}
              maxLength={40}
              placeholder="例如：夜间驾驶"
              aria-label="文件夹名称"
              onChange={(event) => {
                setNewFolder(event.target.value);
                if (createError) setCreateError(null);
              }}
            />
            {createError ? <p className="text-xs text-destructive">{createError}</p> : null}
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setCreateOpen(false)}>取消</Button>
              <Button type="submit">创建文件夹</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
