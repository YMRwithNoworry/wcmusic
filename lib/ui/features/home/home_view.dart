import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../domain/models/track.dart';
import '../../core/organic_artwork.dart';
import '../player/player_view_model.dart';

class HomeView extends StatelessWidget {
  const HomeView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final featured = viewModel.tracks.isEmpty ? null : viewModel.tracks.first;
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Align(
        alignment: Alignment.topLeft,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1160),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _TopLine(trackCount: viewModel.tracks.length),
              const SizedBox(height: 28),
              if (featured != null) _Featured(trackId: featured.id),
              const SizedBox(height: 44),
              const _PlatformPlaylists(),
              const SizedBox(height: 48),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      '最近发布的新曲',
                      style: Theme.of(context).textTheme.headlineLarge,
                    ),
                  ),
                  IconButton(
                    onPressed: viewModel.recentTracks.isEmpty
                        ? viewModel.refreshRecentTracks
                        : viewModel.shuffleRecentTracks,
                    icon: const Icon(Icons.casino_outlined),
                    tooltip: '换一批',
                  ),
                ],
              ),
              const SizedBox(height: 16),
              if (viewModel.isLoadingRecentTracks)
                const LinearProgressIndicator()
              else if (viewModel.recentTracks.isEmpty)
                InkWell(
                  onTap: viewModel.refreshRecentTracks,
                  borderRadius: BorderRadius.circular(8),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 24),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        const Icon(Icons.cloud_off_outlined),
                        const SizedBox(width: 10),
                        Flexible(
                          child: Text(
                            viewModel.recentTracksError ?? '点击载入最近新曲',
                          ),
                        ),
                      ],
                    ),
                  ),
                )
              else
                LayoutBuilder(
                  builder: (context, constraints) {
                    final count = constraints.maxWidth > 800
                        ? 4
                        : constraints.maxWidth > 520
                        ? 3
                        : 2;
                    final items = viewModel.recentTracks.take(count).toList();
                    return GridView.builder(
                      shrinkWrap: true,
                      physics: const NeverScrollableScrollPhysics(),
                      gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                        crossAxisCount: count,
                        crossAxisSpacing: 14,
                        mainAxisSpacing: 14,
                        childAspectRatio: .82,
                      ),
                      itemCount: items.length,
                      itemBuilder: (context, index) {
                        final track = items[index];
                        final released = track.releaseDate;
                        return _AlbumTile(
                          title: track.title,
                          artist: released == null
                              ? track.artist
                              : '${track.artist} · ${released.month}/${released.day}',
                          seed: track.id,
                          artworkUri: track.artworkUri,
                          onTap: () => viewModel.playTrack(track),
                        );
                      },
                    );
                  },
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PlatformPlaylists extends StatelessWidget {
  const _PlatformPlaylists();

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                '平台热门歌单',
                style: Theme.of(context).textTheme.headlineLarge,
              ),
            ),
            IconButton(
              onPressed: viewModel.platformPlaylists.isEmpty
                  ? viewModel.refreshPlatformPlaylists
                  : viewModel.shufflePlatformPlaylists,
              icon: const Icon(Icons.casino_outlined),
              tooltip: '换一批',
            ),
          ],
        ),
        const SizedBox(height: 14),
        if (viewModel.isLoadingPlatformPlaylists)
          const LinearProgressIndicator()
        else if (viewModel.platformPlaylists.isEmpty)
          InkWell(
            onTap: viewModel.refreshPlatformPlaylists,
            borderRadius: BorderRadius.circular(8),
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 24),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(Icons.cloud_off_outlined),
                  const SizedBox(width: 10),
                  Text(viewModel.platformPlaylistError ?? '点击载入平台歌单'),
                ],
              ),
            ),
          )
        else
          LayoutBuilder(
            builder: (context, constraints) {
              final count = constraints.maxWidth > 800
                  ? 4
                  : constraints.maxWidth > 520
                  ? 3
                  : 2;
              return GridView.builder(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: count,
                  crossAxisSpacing: 14,
                  mainAxisSpacing: 14,
                  childAspectRatio: .82,
                ),
                itemCount: viewModel.platformPlaylists.take(count).length,
                itemBuilder: (context, index) {
                  final playlist = viewModel.platformPlaylists[index];
                  return _PlatformPlaylistTile(
                    playlist: playlist,
                    isFavorite: viewModel.isPlatformPlaylistFavorite(playlist),
                    isLoading: viewModel.isPlatformPlaylistLoading(playlist),
                    onTap: () => _open(context, playlist.url),
                    onToggleFavorite: () =>
                        viewModel.togglePlatformPlaylistFavorite(playlist),
                  );
                },
              );
            },
          ),
      ],
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

class _PlatformPlaylistTile extends StatelessWidget {
  const _PlatformPlaylistTile({
    required this.playlist,
    required this.isFavorite,
    required this.isLoading,
    required this.onTap,
    required this.onToggleFavorite,
  });

  final PlatformPlaylist playlist;
  final bool isFavorite;
  final bool isLoading;
  final VoidCallback onTap;
  final VoidCallback onToggleFavorite;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Stack(
              fit: StackFit.expand,
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: playlist.artworkUri.isEmpty
                      ? OrganicArtwork(seed: playlist.id, size: double.infinity)
                      : Image.network(
                          playlist.artworkUri,
                          width: double.infinity,
                          height: double.infinity,
                          fit: BoxFit.cover,
                          errorBuilder: (_, _, _) => OrganicArtwork(
                            seed: playlist.id,
                            size: double.infinity,
                          ),
                        ),
                ),
                Positioned(
                  top: 8,
                  right: 8,
                  child: IconButton.filledTonal(
                    onPressed: isLoading ? null : onToggleFavorite,
                    icon: isLoading
                        ? const SizedBox.square(
                            dimension: 20,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : Icon(
                            isFavorite ? Icons.bookmark : Icons.bookmark_border,
                          ),
                    tooltip: isLoading
                        ? '正在载入歌单'
                        : isFavorite
                        ? '取消收藏'
                        : '收藏并载入歌曲',
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 10),
          Text(
            playlist.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          Text(playlist.platform, maxLines: 1),
        ],
      ),
    );
  }
}

class _TopLine extends StatelessWidget {
  const _TopLine({required this.trackCount});

  final int trackCount;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: Text(
            '让声音自然生长',
            style: Theme.of(context).textTheme.displayLarge,
            maxLines: 2,
          ),
        ),
        const SizedBox(width: 18),
        Text('$trackCount 首', style: Theme.of(context).textTheme.bodyLarge),
      ],
    );
  }
}

class _Featured extends StatelessWidget {
  const _Featured({required this.trackId});

  final String trackId;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final track = viewModel.tracks.firstWhere((item) => item.id == trackId);
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 660;
        final art = OrganicArtwork(
          seed: track.id,
          size: compact ? constraints.maxWidth : 300,
          playing: viewModel.isPlaying,
        );
        final copy = Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(track.album, style: Theme.of(context).textTheme.bodyLarge),
            const SizedBox(height: 12),
            Text(
              track.title,
              style: Theme.of(context).textTheme.displayMedium,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            const SizedBox(height: 8),
            Text(track.artist, style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 26),
            FilledButton.icon(
              onPressed: () => viewModel.playTrack(track),
              icon: const Icon(Icons.play_arrow),
              label: const Text('播放'),
            ),
          ],
        );
        return Container(
          padding: EdgeInsets.all(compact ? 16 : 22),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surface,
            borderRadius: BorderRadius.circular(8),
          ),
          child: compact
              ? Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [art, const SizedBox(height: 22), copy],
                )
              : Row(
                  children: [
                    art,
                    const SizedBox(width: 42),
                    Expanded(child: copy),
                  ],
                ),
        );
      },
    );
  }
}

class _AlbumTile extends StatefulWidget {
  const _AlbumTile({
    required this.title,
    required this.artist,
    required this.seed,
    required this.onTap,
    this.artworkUri,
  });

  final String title;
  final String artist;
  final String seed;
  final String? artworkUri;
  final VoidCallback onTap;

  @override
  State<_AlbumTile> createState() => _AlbumTileState();
}

class _AlbumTileState extends State<_AlbumTile> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: AnimatedScale(
                scale: _hovered ? 1.025 : 1,
                duration: const Duration(milliseconds: 260),
                curve: Curves.easeOutCubic,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: widget.artworkUri == null || widget.artworkUri!.isEmpty
                      ? OrganicArtwork(seed: widget.seed, size: double.infinity)
                      : Image.network(
                          widget.artworkUri!,
                          width: double.infinity,
                          height: double.infinity,
                          fit: BoxFit.cover,
                          errorBuilder: (_, _, _) => OrganicArtwork(
                            seed: widget.seed,
                            size: double.infinity,
                          ),
                        ),
                ),
              ),
            ),
            const SizedBox(height: 10),
            Text(
              widget.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 2),
            Text(widget.artist, maxLines: 1, overflow: TextOverflow.ellipsis),
          ],
        ),
      ),
    );
  }
}
