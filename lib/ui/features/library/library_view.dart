import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../domain/models/track.dart';
import '../../core/organic_artwork.dart';
import '../../core/page_scaffold.dart';
import '../../core/track_favorite_menu.dart';
import '../player/player_view_model.dart';

class LibraryView extends StatelessWidget {
  const LibraryView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return PageScaffold(
      title: '曲库',
      subtitle: '${viewModel.tracks.length} 首声音，安静地留在这里',
      actions: [
        IconButton.filledTonal(
          onPressed: () => _createFolder(context),
          icon: const Icon(Icons.create_new_folder_outlined),
          tooltip: '新建文件夹',
        ),
        IconButton.filledTonal(
          onPressed: () => _pickAudio(context),
          icon: const Icon(Icons.add),
          tooltip: '添加本地音乐',
        ),
      ],
      child: Column(
        children: [
          TextField(
            onChanged: viewModel.setQuery,
            decoration: const InputDecoration(
              prefixIcon: Icon(Icons.search),
              hintText: '搜索歌曲、艺术家或专辑',
            ),
          ),
          if (viewModel.folders.isNotEmpty) ...[
            const SizedBox(height: 12),
            _FolderBar(viewModel: viewModel),
          ],
          const SizedBox(height: 18),
          if (viewModel.visibleTracks.isEmpty)
            const _EmptyLibrary()
          else
            for (final track in viewModel.visibleTracks)
              _TrackRow(track: track),
        ],
      ),
    );
  }

  Future<void> _pickAudio(BuildContext context) async {
    final viewModel = context.read<PlayerViewModel>();
    const audioTypes = XTypeGroup(
      label: '音频',
      extensions: ['mp3', 'flac', 'wav', 'm4a', 'aac', 'ogg', 'opus'],
      mimeTypes: ['audio/*'],
    );
    final files = await openFiles(acceptedTypeGroups: const [audioTypes]);
    await viewModel.importLocalAudio(
      files.map((file) => file.path).where((path) => path.isNotEmpty).toList(),
    );
  }

  Future<void> _createFolder(BuildContext context) async {
    final name = await _promptFolderName(context);
    if (name == null) return;
    if (!context.mounted) return;
    final viewModel = context.read<PlayerViewModel>();
    await viewModel.createFolder(name);
    if (context.mounted && viewModel.message != null) {
      ScaffoldMessenger.of(context)
        ..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(viewModel.message!)));
    }
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
            child: const Text('创建'),
          ),
        ],
      ),
    );
    controller.dispose();
    return name == null || name.isEmpty ? null : name;
  }
}

class _FolderBar extends StatelessWidget {
  const _FolderBar({required this.viewModel});

  final PlayerViewModel viewModel;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          ChoiceChip(
            label: const Text('全部歌曲'),
            selected: viewModel.selectedFolderId == null,
            onSelected: (_) => viewModel.selectFolder(null),
          ),
          for (final folder in viewModel.folders) ...[
            const SizedBox(width: 8),
            InputChip(
              label: Text('${folder.name} (${folder.trackIds.length})'),
              selected: viewModel.selectedFolderId == folder.id,
              onSelected: (_) => viewModel.selectFolder(folder.id),
              onDeleted: () => _confirmDelete(context, folder.id),
              deleteButtonTooltipMessage: '删除文件夹',
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _confirmDelete(BuildContext context, String id) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('删除文件夹？'),
        content: const Text('只会移除文件夹，不会删除歌曲。'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed == true) await viewModel.deleteFolder(id);
  }
}

class _TrackRow extends StatelessWidget {
  const _TrackRow({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.read<PlayerViewModel>();
    return InkWell(
      onTap: () => viewModel.playTrack(track),
      onSecondaryTap: () => showTrackFavoriteMenu(
        context,
        track,
        removableFolderId: viewModel.selectedFolderId,
      ),
      onLongPress: () => showTrackFavoriteMenu(
        context,
        track,
        removableFolderId: viewModel.selectedFolderId,
      ),
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 9, horizontal: 8),
        child: Row(
          children: [
            OrganicArtwork(seed: track.id, size: 48),
            const SizedBox(width: 14),
            Expanded(
              flex: 3,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    track.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  Text(
                    track.artist,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
            if (MediaQuery.sizeOf(context).width > 650)
              Expanded(
                child: Text(
                  track.album,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            const SizedBox(width: 12),
            Text(_duration(track.duration)),
            IconButton(
              onPressed: () => showTrackFavoriteMenu(
                context,
                track,
                removableFolderId: viewModel.selectedFolderId,
              ),
              icon: const Icon(Icons.more_horiz),
              tooltip: '更多',
            ),
          ],
        ),
      ),
    );
  }

  String _duration(Duration duration) {
    if (duration == Duration.zero) return '--:--';
    return '${duration.inMinutes}:${(duration.inSeconds % 60).toString().padLeft(2, '0')}';
  }
}

class _EmptyLibrary extends StatelessWidget {
  const _EmptyLibrary();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 72),
      child: Column(
        children: [
          Icon(
            Icons.music_note,
            size: 42,
            color: Theme.of(context).colorScheme.primary,
          ),
          const SizedBox(height: 16),
          Text('没有找到声音', style: Theme.of(context).textTheme.headlineMedium),
        ],
      ),
    );
  }
}
