import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../domain/models/track.dart';
import '../../core/organic_artwork.dart';
import '../../core/remote_artwork.dart';
import '../../core/page_scaffold.dart';
import '../../core/track_favorite_menu.dart';
import '../player/player_view_model.dart';

class PlaylistsView extends StatelessWidget {
  const PlaylistsView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return PageScaffold(
      title: '歌单',
      subtitle: '本地歌单与收藏的平台歌单都留在这里',
      actions: [
        FilledButton.tonalIcon(
          onPressed: () => _import(context),
          icon: const Icon(Icons.file_open),
          label: const Text('导入'),
        ),
      ],
      child: LayoutBuilder(
        builder: (context, constraints) {
          if (viewModel.playlists.isEmpty) return const _EmptyPlaylists();
          final columns = constraints.maxWidth >= 900
              ? 3
              : constraints.maxWidth >= 560
              ? 2
              : 1;
          return GridView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
              crossAxisCount: columns,
              crossAxisSpacing: 14,
              mainAxisSpacing: 14,
              childAspectRatio: 2.15,
            ),
            itemCount: viewModel.playlists.length,
            itemBuilder: (context, index) {
              final playlist = viewModel.playlists[index];
              return InkWell(
                onTap: playlist.tracks.isNotEmpty
                    ? () => _showPlaylist(context, playlist)
                    : playlist.isPlatformFavorite
                    ? () => _open(context, playlist.externalUrl!)
                    : null,
                borderRadius: BorderRadius.circular(8),
                child: Ink(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.surface,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Row(
                    children: [
                      ClipRRect(
                        borderRadius: BorderRadius.circular(8),
                        child:
                            playlist.artworkUri == null ||
                                playlist.artworkUri!.isEmpty
                            ? OrganicArtwork(seed: playlist.id, size: 82)
                          : SizedBox.square(
                              dimension: 82,
                              child: RemoteArtwork(
                                url: playlist.artworkUri!,
                                seed: playlist.id,
                              ),
                              ),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Text(
                              playlist.name,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.titleLarge,
                            ),
                            const SizedBox(height: 6),
                            Text(
                              playlist.isPlatformFavorite
                                  ? '${playlist.platform ?? '平台'} · 已收藏'
                                  : '${playlist.tracks.length} 首',
                            ),
                          ],
                        ),
                      ),
                      if (playlist.isPlatformFavorite)
                        IconButton(
                          onPressed: () =>
                              viewModel.removeSavedPlaylist(playlist),
                          icon: const Icon(Icons.bookmark),
                          tooltip: '取消收藏',
                        )
                      else
                        const Icon(Icons.chevron_right),
                    ],
                  ),
                ),
              );
            },
          );
        },
      ),
    );
  }

  Future<void> _import(BuildContext context) async {
    const playlistTypes = XTypeGroup(
      label: '歌单',
      extensions: ['m3u', 'm3u8', 'json'],
      mimeTypes: ['audio/x-mpegurl', 'application/json'],
    );
    final file = await openFile(acceptedTypeGroups: const [playlistTypes]);
    if (file == null) return;
    final bytes = await file.readAsBytes();
    if (!context.mounted) return;
    await context.read<PlayerViewModel>().importPlaylist(file.path, bytes);
  }

  Future<void> _showPlaylist(BuildContext context, Playlist playlist) {
    final viewModel = context.read<PlayerViewModel>();
    return showModalBottomSheet<void>(
      context: context,
      useSafeArea: true,
      isScrollControlled: true,
      builder: (sheetContext) => FractionallySizedBox(
        heightFactor: .82,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 18, 12, 12),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          playlist.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(
                            sheetContext,
                          ).textTheme.headlineMedium,
                        ),
                        Text(
                          playlist.isPlatformFavorite
                              ? '${playlist.platform ?? '平台'} · ${playlist.tracks.length} 首可播放'
                              : '${playlist.tracks.length} 首',
                        ),
                      ],
                    ),
                  ),
                  if (playlist.isPlatformFavorite)
                    IconButton(
                      onPressed: () =>
                          _open(sheetContext, playlist.externalUrl!),
                      icon: const Icon(Icons.open_in_new),
                      tooltip: '打开平台页面',
                    ),
                  IconButton(
                    onPressed: () => Navigator.of(sheetContext).pop(),
                    icon: const Icon(Icons.close),
                    tooltip: '关闭',
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: ListView.separated(
                padding: const EdgeInsets.symmetric(vertical: 8),
                itemCount: playlist.tracks.length,
                separatorBuilder: (_, _) => const Divider(height: 1),
                itemBuilder: (context, index) {
                  final track = playlist.tracks[index];
                  return GestureDetector(
                    onSecondaryTap: () =>
                        showTrackFavoriteMenu(sheetContext, track),
                    child: ListTile(
                      leading: ClipRRect(
                        borderRadius: BorderRadius.circular(6),
                        child:
                            track.artworkUri == null ||
                                track.artworkUri!.isEmpty
                            ? OrganicArtwork(seed: track.id, size: 48)
                          : SizedBox.square(
                              dimension: 48,
                              child: RemoteArtwork(
                                url: track.artworkUri!,
                                seed: track.id,
                              ),
                              ),
                      ),
                      title: Text(
                        track.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      subtitle: Text(
                        track.artist,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      trailing: IconButton(
                        onPressed: () =>
                            showTrackFavoriteMenu(sheetContext, track),
                        icon: const Icon(Icons.more_horiz),
                        tooltip: '更多',
                      ),
                      onLongPress: () =>
                          showTrackFavoriteMenu(sheetContext, track),
                      onTap: () {
                        Navigator.of(sheetContext).pop();
                        viewModel.playTrack(track);
                      },
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _open(BuildContext context, String value) async {
    final opened = await launchUrl(
      Uri.parse(value),
      mode: LaunchMode.externalApplication,
    );
    if (!opened && context.mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('无法打开平台歌单')));
    }
  }
}

class _EmptyPlaylists extends StatelessWidget {
  const _EmptyPlaylists();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 70),
      child: Center(
        child: Text(
          '导入一份歌单，让熟悉的声音在这里重新生长',
          style: Theme.of(context).textTheme.titleLarge,
        ),
      ),
    );
  }
}
