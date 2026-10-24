import 'dart:ui' show ImageFilter;

import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import '../nav.dart';
import '../widgets/artwork.dart';
import '../widgets/common.dart';
import '../widgets/dialogs.dart';
import '../widgets/hover.dart';
import '../widgets/track_actions.dart';
import '../widgets/track_list.dart';

/// 发现页：精选、平台歌单、最近新曲。
class HomePage extends StatelessWidget {
  const HomePage({super.key, required this.onNavigate});

  final ValueChanged<AppSection> onNavigate;

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final featured = model.tracks.isEmpty ? null : model.tracks.first;
    return ListView(
      padding: const EdgeInsets.only(bottom: 32),
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 每日发现',
          title: '此刻，听见喜欢',
          subtitle: model.tracks.isEmpty
              ? '从平台歌单里挑一张，或者导入自己的音乐'
              : '曲库里已经有 ${model.tracks.length} 首声音，慢慢听',
          actions: [
            FButton(
              variant: FButtonVariant.outline,
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.sparkles, size: 15),
              onPress: () {
                model.shufflePlatformPlaylists();
                model.shuffleRecentTracks();
              },
              child: const Text('换一批'),
            ),
          ],
        ),
        if (featured != null) ...[
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 32),
            child: PageBody(child: _FeaturedCard(track: featured)),
          ),
          const SizedBox(height: 34),
        ],
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: PageBody(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                SectionTitle(
                  title: '平台热门歌单',
                  caption: '来自各平台的每日精选',
                  action: RoundAction(
                    icon: FLucideIcons.refreshCw,
                    tooltip: '换一批歌单',
                    onPress: model.platformPlaylists.isEmpty
                        ? model.refreshPlatformPlaylists
                        : model.shufflePlatformPlaylists,
                  ),
                ),
                _PlatformPlaylists(
                  onOpenAll: () => onNavigate(AppSection.playlists),
                ),
                const SizedBox(height: 36),
                SectionTitle(
                  title: '最近发布的新曲',
                  caption: '新鲜声音，按发布日排序',
                  action: RoundAction(
                    icon: FLucideIcons.refreshCw,
                    tooltip: '换一批新曲',
                    onPress: model.recentTracks.isEmpty
                        ? model.refreshRecentTracks
                        : model.shuffleRecentTracks,
                  ),
                ),
                _RecentTracks(),
                const SizedBox(height: 36),
                SectionTitle(
                  title: '继续整理',
                  caption: '把喜欢的声音收进自己的曲库',
                ),
                _QuickActions(onNavigate: onNavigate),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class RoundAction extends StatelessWidget {
  const RoundAction({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.onPress,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback onPress;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    return FTooltip(
      tipBuilder: (context, _) => Text(tooltip),
      child: FTappable(
        onPress: onPress,
        builder: (context, variants, _) => Container(
          width: 30,
          height: 30,
          decoration: BoxDecoration(
            color: variants.contains(FTappableVariant.hovered)
                ? colors.secondary
                : const Color(0x00000000),
            borderRadius: const BorderRadius.all(Radius.circular(9)),
          ),
          child: Icon(icon, size: 15, color: colors.mutedForeground),
        ),
      ),
    );
  }
}

class _FeaturedCard extends StatelessWidget {
  const _FeaturedCard({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final model = context.watch<PlayerViewModel>();
    final playing = model.current?.id == track.id && model.isPlaying;
    final compact = MediaQuery.sizeOf(context).width < 760;
    return ClipRRect(
      borderRadius: const BorderRadius.all(Radius.circular(20)),
      child: SizedBox(
        height: compact ? 176 : 236,
        child: Stack(
          fit: StackFit.expand,
          children: [
            RepaintBoundary(
              child: ImageFiltered(
                imageFilter: ImageFilter.blur(sigmaX: 26, sigmaY: 26),
                child: Transform.scale(
                  scale: 1.18,
                  child: (track.artworkUri == null ||
                          track.artworkUri!.isEmpty)
                      ? _CoverFill(seed: track.id)
                      : Image.network(
                          track.artworkUri!,
                          fit: BoxFit.cover,
                          filterQuality: FilterQuality.low,
                          headers: const {
                            'User-Agent': 'Mozilla/5.0',
                            'Referer': 'https://music.163.com/',
                          },
                          errorBuilder: (_, _, _) =>
                              _CoverFill(seed: track.id),
                        ),
                ),
              ),
            ),
            DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.centerLeft,
                  end: Alignment.centerRight,
                  colors: [
                    colors.card.withValues(alpha: .96),
                    colors.card.withValues(alpha: .78),
                    colors.card.withValues(alpha: .42),
                  ],
                ),
              ),
            ),
            Padding(
              padding: EdgeInsets.all(compact ? 16 : 22),
              child: Row(
                children: [
                  ArtworkView(
                    seed: track.id,
                    url: track.artworkUri,
                    size: compact ? 128 : 190,
                    radius: 14,
                    playing: playing,
                  ),
                  SizedBox(width: compact ? 16 : 30),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (!compact) ...[
                          Text(
                            track.album.isEmpty ? '精选单曲' : track.album,
                            style: type.body.xs.copyWith(
                              color: colors.primary,
                              fontWeight: FontWeight.w600,
                              letterSpacing: 1.4,
                            ),
                          ),
                          const SizedBox(height: 12),
                        ],
                        Text(
                          track.title,
                          maxLines: compact ? 1 : 2,
                          overflow: TextOverflow.ellipsis,
                          style: (compact ? type.display.xl : type.display.xl4).copyWith(
                            color: colors.foreground,
                            fontWeight: FontWeight.w600,
                            letterSpacing: -.8,
                            height: 1.1,
                          ),
                        ),
                        SizedBox(height: compact ? 4 : 8),
                        Text(
                          track.artist,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: type.body.md.copyWith(
                            color: colors.mutedForeground,
                          ),
                        ),
                        SizedBox(height: compact ? 12 : 22),
                        Wrap(
                          spacing: 10,
                          runSpacing: 8,
                          children: [
                            FButton(
                              size: FButtonSizeVariant.md,
                              mainAxisSize: MainAxisSize.min,
                              prefix: Icon(
                                playing ? FLucideIcons.pause : FLucideIcons.play,
                                size: 16,
                              ),
                              onPress: model.current?.id == track.id
                                  ? model.togglePlayback
                                  : () => model.playTrack(track),
                              child: Text(playing ? '暂停' : '播放'),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

}

class _CoverFill extends StatelessWidget {
  const _CoverFill({required this.seed});

  final String seed;

  @override
  Widget build(BuildContext context) => ArtworkView(
    seed: seed,
    size: double.infinity,
    radius: 0,
  );
}

class _PlatformPlaylists extends StatelessWidget {
  const _PlatformPlaylists({required this.onOpenAll});

  final VoidCallback onOpenAll;

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    if (model.isLoadingPlatformPlaylists && model.platformPlaylists.isEmpty) {
      return const LoadingState(label: '正在载入平台歌单');
    }
    if (model.platformPlaylists.isEmpty) {
      return ErrorState(
        message: model.platformPlaylistError ?? '平台歌单暂时不可用',
        onRetry: model.refreshPlatformPlaylists,
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 1080
            ? 4
            : constraints.maxWidth >= 760
            ? 3
            : 2;
        return GridView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          padding: EdgeInsets.zero,
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columns,
            crossAxisSpacing: 16,
            mainAxisSpacing: 18,
            childAspectRatio: .74,
          ),
          itemCount: model.platformPlaylists.take(columns).length,
          itemBuilder: (context, index) {
            final playlist = model.platformPlaylists[index];
            return _PlaylistCard(
              playlist: playlist,
              isFavorite: model.isPlatformPlaylistFavorite(playlist),
              isLoading: model.isPlatformPlaylistLoading(playlist),
              onOpen: () => _openPlaylist(context, playlist),
              onToggleFavorite: () =>
                  model.togglePlatformPlaylistFavorite(playlist),
            );
          },
        );
      },
    );
  }

  Future<void> _openPlaylist(
    BuildContext context,
    PlatformPlaylist playlist,
  ) async {
    final model = context.read<PlayerViewModel>();
    final tracks = await model.loadPlatformPlaylistTracks(playlist);
    if (!context.mounted) return;
    if (tracks == null) {
      showAppToast(context, model.message ?? '歌单暂时不可用', error: true);
      return;
    }
    await showFSheet<void>(
      context: context,
      side: FLayout.rtl,
      mainAxisMaxRatio: null,
      constraints: const BoxConstraints(minWidth: 420, maxWidth: 560),
      builder: (sheetContext) => _PlaylistTracksSheet(
        name: playlist.name,
        caption: '${playlist.platform} · ${tracks.length} 首',
        artworkUri: playlist.artworkUri,
        seed: playlist.id,
        tracks: tracks,
        onOpenExternal: playlist.url.isEmpty
            ? null
            : () => launchUrl(
                Uri.parse(playlist.url),
                mode: LaunchMode.externalApplication,
              ),
      ),
    );
  }
}

class _PlaylistCard extends StatelessWidget {
  const _PlaylistCard({
    required this.playlist,
    required this.isFavorite,
    required this.isLoading,
    required this.onOpen,
    required this.onToggleFavorite,
  });

  final PlatformPlaylist playlist;
  final bool isFavorite;
  final bool isLoading;
  final VoidCallback onOpen;
  final VoidCallback onToggleFavorite;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return HoverLift(
      onTap: onOpen,
      borderRadius: const BorderRadius.all(Radius.circular(16)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Stack(
              fit: StackFit.expand,
              children: [
                ArtworkView(
                  seed: playlist.id,
                  url: playlist.artworkUri,
                  size: double.infinity,
                  radius: 16,
                ),
                Positioned(
                  top: 10,
                  right: 10,
                  child: FTooltip(
                    tipBuilder: (context, _) => Text(
                      isLoading
                          ? '正在载入歌单'
                          : isFavorite
                          ? '取消收藏'
                          : '收藏并载入歌曲',
                    ),
                    child: FTappable(
                      onPress: isLoading ? null : onToggleFavorite,
                      builder: (context, variants, _) => AnimatedContainer(
                        duration: const Duration(milliseconds: 160),
                        width: 34,
                        height: 34,
                        decoration: BoxDecoration(
                          color: isFavorite
                              ? colors.primary
                              : colors.card.withValues(alpha: .82),
                          shape: BoxShape.circle,
                          border: Border.all(
                            color: colors.border,
                            width: isFavorite ? 0 : 1,
                          ),
                        ),
                        child: isLoading
                            ? const Padding(
                                padding: EdgeInsets.all(9),
                                child: FCircularProgress(
                                  size: FCircularProgressSizeVariant.xs,
                                ),
                              )
                            : Icon(
                                isFavorite
                                    ? FLucideIcons.bookmark
                                    : FLucideIcons.plus,
                                size: 15,
                                color: isFavorite
                                    ? colors.primaryForeground
                                    : colors.foreground,
                              ),
                      ),
                    ),
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
            style: type.body.sm.copyWith(
              color: colors.foreground,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            playlist.platform,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: type.body.xs.copyWith(color: colors.mutedForeground),
          ),
        ],
      ),
    );
  }
}

class _RecentTracks extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    if (model.isLoadingRecentTracks && model.recentTracks.isEmpty) {
      return const LoadingState(label: '正在载入新曲');
    }
    if (model.recentTracks.isEmpty) {
      return ErrorState(
        message: model.recentTracksError ?? '新曲推荐暂时不可用',
        onRetry: model.refreshRecentTracks,
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 1080
            ? 4
            : constraints.maxWidth >= 760
            ? 3
            : 2;
        return GridView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          padding: EdgeInsets.zero,
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columns,
            crossAxisSpacing: 16,
            mainAxisSpacing: 18,
            childAspectRatio: .74,
          ),
          itemCount: model.recentTracks.take(columns).length,
          itemBuilder: (context, index) {
            final track = model.recentTracks[index];
            final active = model.current?.id == track.id;
            return _NewTrackCard(
              track: track,
              active: active,
              playing: active && model.isPlaying,
              onPlay: () => model.playTrack(track),
              onMenu: () => showTrackActions(context, track),
            );
          },
        );
      },
    );
  }
}

class _NewTrackCard extends StatelessWidget {
  const _NewTrackCard({
    required this.track,
    required this.active,
    required this.playing,
    required this.onPlay,
    required this.onMenu,
  });

  final Track track;
  final bool active;
  final bool playing;
  final VoidCallback onPlay;
  final VoidCallback onMenu;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final released = track.releaseDate;
    return HoverLift(
      onTap: onPlay,
      onSecondaryTap: onMenu,
      onLongPress: onMenu,
      borderRadius: const BorderRadius.all(Radius.circular(16)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Stack(
              fit: StackFit.expand,
              children: [
                ArtworkView(
                  seed: track.id,
                  url: track.artworkUri,
                  size: double.infinity,
                  radius: 16,
                  playing: playing,
                ),
                if (active)
                  Positioned(
                    left: 10,
                    bottom: 10,
                    child: Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 4,
                      ),
                      decoration: BoxDecoration(
                        color: colors.card.withValues(alpha: .86),
                        borderRadius: const BorderRadius.all(
                          Radius.circular(999),
                        ),
                      ),
                      child: PlayingBars(playing: playing, height: 12),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 10),
          Text(
            track.title,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: type.body.sm.copyWith(
              color: active ? colors.primary : colors.foreground,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            released == null
                ? track.artist
                : '${track.artist} · ${released.month}/${released.day}',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: type.body.xs.copyWith(color: colors.mutedForeground),
          ),
        ],
      ),
    );
  }
}

class _QuickActions extends StatelessWidget {
  const _QuickActions({required this.onNavigate});

  final ValueChanged<AppSection> onNavigate;

  @override
  Widget build(BuildContext context) {
    const entries = [
      (AppSection.library, '整理曲库', '导入本地音乐、建立文件夹'),
      (AppSection.playlists, '查看歌单', '收藏的平台歌单都在这里'),
      (AppSection.sources, '管理音源', '导入洛雪音源脚本'),
      (AppSection.settings, '调整偏好', '主题、歌词与播放行为'),
    ];
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 980 ? 4 : 2;
        return GridView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          padding: EdgeInsets.zero,
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columns,
            crossAxisSpacing: 14,
            mainAxisSpacing: 14,
            mainAxisExtent: 80,
          ),
          itemCount: entries.length,
          itemBuilder: (context, index) {
            final entry = entries[index];
            return _QuickActionCard(
              section: entry.$1,
              title: entry.$2,
              caption: entry.$3,
              onTap: () => onNavigate(entry.$1),
            );
          },
        );
      },
    );
  }
}

class _QuickActionCard extends StatelessWidget {
  const _QuickActionCard({
    required this.section,
    required this.title,
    required this.caption,
    required this.onTap,
  });

  final AppSection section;
  final String title;
  final String caption;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return HoverLift(
      scale: 1.01,
      onTap: onTap,
      borderRadius: const BorderRadius.all(Radius.circular(14)),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        decoration: BoxDecoration(
          color: colors.secondary.withValues(alpha: .5),
          borderRadius: const BorderRadius.all(Radius.circular(14)),
          border: Border.all(color: colors.border),
        ),
        child: Row(
          children: [
            Icon(section.icon, size: 18, color: colors.primary),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: type.body.sm.copyWith(
                      color: colors.foreground,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    caption,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: type.body.xs.copyWith(
                      color: colors.mutedForeground,
                    ),
                  ),
                ],
              ),
            ),
            Icon(
              FLucideIcons.chevronRight,
              size: 15,
              color: colors.mutedForeground,
            ),
          ],
        ),
      ),
    );
  }
}

/// 歌单详情面板，供发现页与歌单页复用。
class _PlaylistTracksSheet extends StatelessWidget {
  const _PlaylistTracksSheet({
    required this.name,
    required this.caption,
    required this.artworkUri,
    required this.seed,
    required this.tracks,
    this.onOpenExternal,
  });

  final String name;
  final String caption;
  final String artworkUri;
  final String seed;
  final List<Track> tracks;
  final VoidCallback? onOpenExternal;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final model = context.watch<PlayerViewModel>();
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 20, 12, 16),
          child: Row(
            children: [
              ArtworkView(
                seed: seed,
                url: artworkUri,
                size: 58,
                radius: 12,
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: type.display.xs.copyWith(
                        color: colors.foreground,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      caption,
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                      ),
                    ),
                  ],
                ),
              ),
              if (onOpenExternal != null)
                FButton.icon(
                  variant: FButtonVariant.ghost,
                  size: FButtonSizeVariant.sm,
                  onPress: onOpenExternal,
                  child: const Icon(FLucideIcons.externalLink, size: 16),
                ),
              FButton.icon(
                variant: FButtonVariant.ghost,
                size: FButtonSizeVariant.sm,
                onPress: () => Navigator.of(context).pop(),
                child: const Icon(FLucideIcons.x, size: 16),
              ),
            ],
          ),
        ),
        FDivider(style: .delta(padding: EdgeInsetsGeometryDelta.value(EdgeInsets.zero))),
        Expanded(
          child: TrackListView(
            tracks: tracks,
            currentId: model.current?.id,
            isPlaying: model.isPlaying,
            onPlay: (track) {
              Navigator.of(context).pop();
              model.playTrack(track);
            },
            onMenu: (track) => showTrackActions(context, track),
          ),
        ),
      ],
    );
  }
}

/// 打开歌单详情（供歌单页复用）。
Future<void> showPlaylistSheet(
  BuildContext context, {
  required String name,
  required String caption,
  required String artworkUri,
  required String seed,
  required List<Track> tracks,
  VoidCallback? onOpenExternal,
}) {
  return showFSheet<void>(
    context: context,
    side: FLayout.rtl,
    mainAxisMaxRatio: null,
    constraints: const BoxConstraints(minWidth: 420, maxWidth: 560),
    builder: (sheetContext) => _PlaylistTracksSheet(
      name: name,
      caption: caption,
      artworkUri: artworkUri,
      seed: seed,
      tracks: tracks,
      onOpenExternal: onOpenExternal,
    ),
  );
}
