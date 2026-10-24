import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/music_folder.dart';
import '../../../domain/models/playback_quality.dart';
import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import 'artwork.dart';
import 'dialogs.dart';

sealed class _TrackAction {
  const _TrackAction();
}

class _DownloadAction extends _TrackAction {
  const _DownloadAction(this.quality);

  final PlaybackQuality? quality;
}

class _FavoriteAction extends _TrackAction {
  const _FavoriteAction(this.folderId);

  final String folderId;
}

class _RemoveAction extends _TrackAction {
  const _RemoveAction();
}

class _NewFolderAction extends _TrackAction {
  const _NewFolderAction();
}

/// 曲目操作面板：下载、收藏到文件夹、移出文件夹、新建文件夹。
Future<void> showTrackActions(
  BuildContext context,
  Track track, {
  String? removableFolderId,
}) async {
  final model = context.read<PlayerViewModel>();
  MusicFolder? removable;
  if (removableFolderId != null) {
    for (final folder in model.folders) {
      if (folder.id == removableFolderId) {
        removable = folder;
        break;
      }
    }
  }
  final supportsQuality =
      track.source != TrackSource.local &&
      (track.sourceId?.isNotEmpty ?? false);
  final action = await showFSheet<_TrackAction>(
    context: context,
    side: FLayout.btt,
    mainAxisMaxRatio: .74,
    builder: (sheetContext) {
      final colors = sheetContext.theme.colors;
      final type = sheetContext.theme.typography;
      return Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(18, 16, 12, 12),
            child: Row(
              children: [
                ArtworkView(
                  seed: track.id,
                  url: track.artworkUri,
                  size: 46,
                  radius: 10,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        track.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: type.body.sm.copyWith(
                          color: colors.foreground,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      Text(
                        track.artist,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: type.body.xs.copyWith(
                          color: colors.mutedForeground,
                        ),
                      ),
                    ],
                  ),
                ),
                FButton.icon(
                  variant: FButtonVariant.ghost,
                  size: FButtonSizeVariant.sm,
                  onPress: () => Navigator.of(sheetContext).pop(),
                  child: const Icon(FLucideIcons.x, size: 16),
                ),
              ],
            ),
          ),
          FDivider(style: .delta(padding: EdgeInsetsGeometryDelta.value(EdgeInsets.zero))),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(8, 8, 8, 16),
              children: [
                FItemGroup(
                  divider: FItemDivider.full,
                  children: [
                    if (supportsQuality)
                      for (final quality in PlaybackQuality.values)
                        FItem(
                          title: Text('下载 · ${quality.label}'),
                          prefix: const Icon(FLucideIcons.download, size: 17),
                          onPress: () => Navigator.of(
                            sheetContext,
                          ).pop(_DownloadAction(quality)),
                        )
                    else
                      FItem(
                        title: const Text('下载 · 原始音质'),
                        prefix: const Icon(FLucideIcons.download, size: 17),
                        onPress: () => Navigator.of(
                          sheetContext,
                        ).pop(const _DownloadAction(null)),
                      ),
                    if (removable != null)
                      FItem(
                        title: Text('移出「${removable.name}」'),
                        prefix: const Icon(FLucideIcons.folderMinus, size: 17),
                        onPress: () => Navigator.of(
                          sheetContext,
                        ).pop(const _RemoveAction()),
                      ),
                  ],
                ),
                const SizedBox(height: 10),
                FItemGroup(
                  divider: FItemDivider.full,
                  children: [
                    for (final folder in model.folders)
                      FItem(
                        title: Text(
                          folder.trackIds.contains(track.id)
                              ? '${folder.name}（已收藏）'
                              : '收藏到 ${folder.name}',
                        ),
                        prefix: Icon(
                          folder.trackIds.contains(track.id)
                              ? FLucideIcons.folderCheck
                              : FLucideIcons.folder,
                          size: 17,
                        ),
                        onPress: () => Navigator.of(
                          sheetContext,
                        ).pop(_FavoriteAction(folder.id)),
                      ),
                    FItem(
                      title: const Text('新建文件夹并收藏'),
                      prefix: const Icon(FLucideIcons.folderPlus, size: 17),
                      onPress: () => Navigator.of(
                        sheetContext,
                      ).pop(const _NewFolderAction()),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      );
    },
  );
  if (action == null || !context.mounted) return;
  switch (action) {
    case _DownloadAction(:final quality):
      await model.downloadTrack(track, quality);
    case _FavoriteAction(:final folderId):
      await model.favoriteTrack(folderId, track);
    case _RemoveAction():
      if (removableFolderId != null) {
        await model.removeTrackFromFolder(removableFolderId, track.id);
      }
    case _NewFolderAction():
      if (!context.mounted) return;
      final name = await promptForText(
        context,
        title: '新建文件夹',
        hint: '文件夹名称',
        confirmLabel: '创建并收藏',
      );
      if (name == null || !context.mounted) return;
      final folder = await model.createFolder(name);
      if (folder != null) await model.favoriteTrack(folder.id, track);
  }
}
