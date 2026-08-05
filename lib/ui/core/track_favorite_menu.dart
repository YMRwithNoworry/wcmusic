import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../domain/models/playback_quality.dart';
import '../../domain/models/track.dart';
import '../features/player/player_view_model.dart';

Future<void> showTrackFavoriteMenu(
  BuildContext context,
  Track track, {
  String? removableFolderId,
}) async {
  final viewModel = context.read<PlayerViewModel>();
  final removableFolderIndex = removableFolderId == null
      ? -1
      : viewModel.folders.indexWhere(
          (folder) => folder.id == removableFolderId,
        );
  final removableFolder = removableFolderIndex < 0
      ? null
      : viewModel.folders[removableFolderIndex];
  final supportsQualitySelection =
      track.source != TrackSource.local &&
      (track.sourceId?.isNotEmpty ?? false);
  final action = await showMenu<String>(
    context: context,
    position: _menuPosition(context),
    items: [
      if (supportsQualitySelection)
        for (final quality in PlaybackQuality.values)
          PopupMenuItem(
            value: '__download_${quality.name}',
            child: Row(
              children: [
                const Icon(Icons.download_outlined),
                const SizedBox(width: 12),
                Text('下载 · ${quality.label}'),
              ],
            ),
          )
      else
        const PopupMenuItem(
          value: '__download_original__',
          child: Row(
            children: [
              Icon(Icons.download_outlined),
              SizedBox(width: 12),
              Text('下载 · 原始音质'),
            ],
          ),
        ),
      const PopupMenuDivider(),
      if (removableFolder?.trackIds.contains(track.id) ?? false)
        const PopupMenuItem(
          value: '__remove__',
          child: Row(
            children: [
              Icon(Icons.folder_off_outlined),
              SizedBox(width: 12),
              Text('移出当前文件夹'),
            ],
          ),
        ),
      for (final folder in viewModel.folders)
        PopupMenuItem(
          value: folder.id,
          child: Row(
            children: [
              Icon(
                folder.trackIds.contains(track.id)
                    ? Icons.check_circle_outline
                    : Icons.folder_outlined,
              ),
              const SizedBox(width: 12),
              Flexible(
                child: Text(
                  folder.trackIds.contains(track.id)
                      ? '${folder.name}（已收藏）'
                      : '收藏到 ${folder.name}',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
      const PopupMenuItem(
        value: '__new__',
        child: Row(
          children: [
            Icon(Icons.create_new_folder_outlined),
            SizedBox(width: 12),
            Text('新建文件夹并收藏'),
          ],
        ),
      ),
    ],
  );
  if (action == null || !context.mounted) return;
  if (action == '__download_original__') {
    await viewModel.downloadTrack(track);
    return;
  }
  if (action.startsWith('__download_')) {
    final qualityName = action.substring('__download_'.length);
    final quality = PlaybackQuality.values.firstWhere(
      (item) => item.name == qualityName,
    );
    await viewModel.downloadTrack(track, quality);
    return;
  }
  if (action == '__remove__' && removableFolder != null) {
    await viewModel.removeTrackFromFolder(removableFolder.id, track.id);
    return;
  }
  if (action == '__new__') {
    final name = await _promptFolderName(context);
    if (name == null || !context.mounted) return;
    final folder = await viewModel.createFolder(name);
    if (folder != null) await viewModel.favoriteTrack(folder.id, track);
    return;
  }
  await viewModel.favoriteTrack(action, track);
}

Future<String?> _promptFolderName(BuildContext context) async {
  final controller = TextEditingController();
  final name = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('新建文件夹'),
      content: TextField(
        controller: controller,
        autofocus: true,
        decoration: const InputDecoration(hintText: '文件夹名称'),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, controller.text.trim()),
          child: const Text('创建并收藏'),
        ),
      ],
    ),
  );
  controller.dispose();
  return name == null || name.isEmpty ? null : name;
}

RelativeRect _menuPosition(BuildContext context) {
  final box = context.findRenderObject() as RenderBox?;
  final overlay = Overlay.of(context).context.findRenderObject() as RenderBox?;
  if (box == null || overlay == null || !box.hasSize) return RelativeRect.fill;
  return RelativeRect.fromRect(
    Rect.fromPoints(
      box.localToGlobal(Offset.zero),
      box.localToGlobal(box.size.bottomRight(Offset.zero)),
    ),
    Offset.zero & overlay.size,
  );
}
