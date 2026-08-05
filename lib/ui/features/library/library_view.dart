import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:uuid/uuid.dart';

import '../../../domain/models/track.dart';
import '../../core/organic_artwork.dart';
import '../../core/page_scaffold.dart';
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
    final additions = <Track>[];
    for (final file in files) {
      final path = file.path;
      if (path.isEmpty) continue;
      additions.add(
        Track(
          id: const Uuid().v4(),
          title: file.name.replaceFirst(RegExp(r'\.[^.]+$'), ''),
          artist: '本地音乐',
          album: '最近添加',
          duration: Duration.zero,
          uri: path,
        ),
      );
    }
    await viewModel.addTracks(additions);
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
              onPressed: () {},
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
