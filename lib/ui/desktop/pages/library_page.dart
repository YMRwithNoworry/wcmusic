import 'package:file_selector/file_selector.dart';
import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../state/player_view_model.dart';
import '../widgets/common.dart';
import '../widgets/dialogs.dart';
import '../widgets/track_actions.dart';
import '../widgets/track_list.dart';

class LibraryPage extends StatefulWidget {
  const LibraryPage({super.key});

  @override
  State<LibraryPage> createState() => _LibraryPageState();
}

class _LibraryPageState extends State<LibraryPage> {
  final _searchController = TextEditingController();

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final tracks = model.visibleTracks;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 曲库',
          title: '曲库',
          subtitle: '${model.tracks.length} 首声音 · ${model.folders.length} 个文件夹',
          actions: [
            FButton(
              variant: FButtonVariant.outline,
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.folderPlus, size: 15),
              onPress: _createFolder,
              child: const Text('新建文件夹'),
            ),
            FButton(
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.plus, size: 15),
              onPress: _importAudio,
              child: const Text('添加本地音乐'),
            ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: PageBody(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FTextField(
                  control: FTextFieldControl.managed(
                    controller: _searchController,
                    onChange: (value) => model.setQuery(value.text),
                  ),
                  hint: '搜索歌曲、艺术家或专辑',
                  clearable: (_) => true,
                  prefixBuilder: (context, style, _) =>
                      const Icon(FLucideIcons.search, size: 16),
                ),
                if (model.folders.isNotEmpty) ...[
                  const SizedBox(height: 14),
                  SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(
                      children: [
                        Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: FilterPill(
                            label: '全部歌曲',
                            selected: model.selectedFolderId == null,
                            onPress: () => model.selectFolder(null),
                          ),
                        ),
                        for (final folder in model.folders)
                          Padding(
                            padding: const EdgeInsets.only(right: 8),
                            child: FilterPill(
                              label:
                                  '${folder.name} (${folder.trackIds.length})',
                              selected: model.selectedFolderId == folder.id,
                              onPress: () => model.selectFolder(folder.id),
                              trailing: FTappable(
                                onPress: () => _deleteFolder(folder.id),
                                builder: (context, variants, _) => Icon(
                                  FLucideIcons.x,
                                  size: 12,
                                  color: context.theme.colors.mutedForeground,
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
                const SizedBox(height: 8),
              ],
            ),
          ),
        ),
        Expanded(
          child: PageBody(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 220),
              switchInCurve: Curves.easeOutCubic,
              child: tracks.isEmpty
                  ? EmptyState(
                      key: const ValueKey('empty'),
                      icon: FLucideIcons.music2,
                      title: '没有找到声音',
                      description: model.query.isEmpty
                          ? '导入本地音乐，或者从搜索页把喜欢的歌收藏进来。'
                          : '没有匹配「${model.query}」的歌曲，换个关键词试试。',
                      action: FButton(
                        size: FButtonSizeVariant.sm,
                        mainAxisSize: MainAxisSize.min,
                        prefix: const Icon(FLucideIcons.plus, size: 15),
                        onPress: _importAudio,
                        child: const Text('添加本地音乐'),
                      ),
                    )
                  : TrackListView(
                      key: const ValueKey('tracks'),
                      tracks: tracks,
                      currentId: model.current?.id,
                      isPlaying: model.isPlaying,
                      onPlay: model.playTrack,
                      onMenu: (track) => showTrackActions(
                        context,
                        track,
                        removableFolderId: model.selectedFolderId,
                      ),
                    ),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _importAudio() async {
    final model = context.read<PlayerViewModel>();
    const audioTypes = XTypeGroup(
      label: '音频',
      extensions: ['mp3', 'flac', 'wav', 'm4a', 'aac', 'ogg', 'opus'],
      mimeTypes: ['audio/*'],
    );
    try {
      final files = await openFiles(acceptedTypeGroups: const [audioTypes]);
      final paths = files
          .map((file) => file.path)
          .where((path) => path.isNotEmpty)
          .toList();
      await model.importLocalAudio(paths);
      if (mounted && model.message != null) {
        showAppToast(context, model.message!);
      }
    } on Object catch (error) {
      if (mounted) showAppToast(context, '选择文件失败：$error', error: true);
    }
  }

  Future<void> _createFolder() async {
    final model = context.read<PlayerViewModel>();
    final name = await promptForText(
      context,
      title: '新建文件夹',
      hint: '文件夹名称',
      confirmLabel: '创建',
    );
    if (name == null) return;
    await model.createFolder(name);
    if (mounted && model.message != null) showAppToast(context, model.message!);
  }

  Future<void> _deleteFolder(String id) async {
    final model = context.read<PlayerViewModel>();
    final confirmed = await confirmAction(
      context,
      title: '删除文件夹？',
      description: '只会移除文件夹，不会删除歌曲。',
      confirmLabel: '删除',
      destructive: true,
    );
    if (confirmed) await model.deleteFolder(id);
  }
}
