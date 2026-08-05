import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../core/organic_artwork.dart';
import '../../core/page_scaffold.dart';
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
                onTap: playlist.isPlatformFavorite
                    ? () => _open(context, playlist.externalUrl!)
                    : playlist.tracks.isEmpty
                    ? null
                    : () => viewModel.playTrack(playlist.tracks.first),
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
                            : Image.network(
                                playlist.artworkUri!,
                                width: 82,
                                height: 82,
                                fit: BoxFit.cover,
                                errorBuilder: (_, _, _) =>
                                    OrganicArtwork(seed: playlist.id, size: 82),
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
