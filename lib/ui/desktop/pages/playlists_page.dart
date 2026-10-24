import 'package:file_selector/file_selector.dart';
import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import '../widgets/artwork.dart';
import '../widgets/common.dart';
import '../widgets/dialogs.dart';
import 'home_page.dart' show showPlaylistSheet;

class PlaylistsPage extends StatelessWidget {
  const PlaylistsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 歌单',
          title: '我的歌单',
          subtitle: '本地歌单与收藏的平台歌单都留在这里',
          actions: [
            FButton(
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.filePlus, size: 15),
              onPress: () => _import(context),
              child: const Text('导入歌单'),
            ),
          ],
        ),
        Expanded(
          child: PageBody(
            child: model.playlists.isEmpty
                ? EmptyState(
                    icon: FLucideIcons.listMusic,
                    title: '还没有歌单',
                    description: '导入一份 M3U / JSON 歌单，或者在发现页收藏平台歌单。',
                    action: FButton(
                      variant: FButtonVariant.outline,
                      size: FButtonSizeVariant.sm,
                      mainAxisSize: MainAxisSize.min,
                      prefix: const Icon(FLucideIcons.filePlus, size: 15),
                      onPress: () => _import(context),
                      child: const Text('导入歌单'),
                    ),
                  )
                : GridView.builder(
                    padding: const EdgeInsets.fromLTRB(32, 4, 32, 24),
                    gridDelegate:
                        const SliverGridDelegateWithMaxCrossAxisExtent(
                          maxCrossAxisExtent: 420,
                          mainAxisExtent: 104,
                          crossAxisSpacing: 14,
                          mainAxisSpacing: 14,
                        ),
                    itemCount: model.playlists.length,
                    itemBuilder: (context, index) {
                      final playlist = model.playlists[index];
                      return _PlaylistTile(playlist: playlist);
                    },
                  ),
          ),
        ),
      ],
    );
  }

  Future<void> _import(BuildContext context) async {
    const playlistTypes = XTypeGroup(
      label: '歌单',
      extensions: ['m3u', 'm3u8', 'json'],
      mimeTypes: ['audio/x-mpegurl', 'application/json'],
    );
    final model = context.read<PlayerViewModel>();
    try {
      final file = await openFile(acceptedTypeGroups: const [playlistTypes]);
      if (file == null) return;
      final bytes = await file.readAsBytes();
      await model.importPlaylist(file.path, bytes);
      if (context.mounted && model.message != null) {
        showAppToast(context, model.message!);
      }
    } on Object catch (error) {
      if (context.mounted) {
        showAppToast(context, '导入失败：$error', error: true);
      }
    }
  }
}

class _PlaylistTile extends StatefulWidget {
  const _PlaylistTile({required this.playlist});

  final Playlist playlist;

  @override
  State<_PlaylistTile> createState() => _PlaylistTileState();
}

class _PlaylistTileState extends State<_PlaylistTile> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final model = context.read<PlayerViewModel>();
    final playlist = widget.playlist;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: () => _open(context, playlist),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 170),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: _hovered
                ? colors.secondary.withValues(alpha: .85)
                : colors.secondary.withValues(alpha: .45),
            borderRadius: const BorderRadius.all(Radius.circular(16)),
            border: Border.all(
              color: _hovered
                  ? colors.primary.withValues(alpha: .35)
                  : colors.border,
            ),
          ),
          child: Row(
            children: [
              ArtworkView(
                seed: playlist.id,
                url: playlist.artworkUri,
                size: 78,
                radius: 12,
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      playlist.name,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.sm.copyWith(
                        color: colors.foreground,
                        fontWeight: FontWeight.w600,
                        height: 1.3,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      playlist.isPlatformFavorite
                          ? '${playlist.platform ?? '平台'} · 已收藏 · ${playlist.tracks.length} 首'
                          : '${playlist.tracks.length} 首',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                      ),
                    ),
                  ],
                ),
              ),
              if (playlist.isPlatformFavorite)
                FTooltip(
                  tipBuilder: (context, _) => const Text('取消收藏'),
                  child: FButton.icon(
                    variant: FButtonVariant.ghost,
                    size: FButtonSizeVariant.sm,
                    onPress: () => model.removeSavedPlaylist(playlist),
                    child: const Icon(FLucideIcons.bookmarkCheck, size: 16),
                  ),
                )
              else
                Icon(
                  FLucideIcons.chevronRight,
                  size: 16,
                  color: colors.mutedForeground,
                ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _open(BuildContext context, Playlist playlist) async {
    final model = context.read<PlayerViewModel>();
    if (playlist.tracks.isNotEmpty) {
      await showPlaylistSheet(
        context,
        name: playlist.name,
        caption: playlist.isPlatformFavorite
            ? '${playlist.platform ?? '平台'} · ${playlist.tracks.length} 首可播放'
            : '${playlist.tracks.length} 首',
        artworkUri: playlist.artworkUri ?? '',
        seed: playlist.id,
        tracks: playlist.tracks,
        onOpenExternal: playlist.externalUrl == null
            ? null
            : () => launchUrl(
                Uri.parse(playlist.externalUrl!),
                mode: LaunchMode.externalApplication,
              ),
      );
      return;
    }
    final url = playlist.externalUrl;
    if (url == null) return;
    final opened = await launchUrl(
      Uri.parse(url),
      mode: LaunchMode.externalApplication,
    );
    if (!opened && context.mounted) {
      showAppToast(context, '无法打开平台歌单', error: true);
    }
    if (model.message != null && context.mounted) {
      showAppToast(context, model.message!);
    }
  }
}
