import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/track.dart';
import '../../../data/services/online_search_service.dart';
import '../../core/organic_artwork.dart';
import '../../core/page_scaffold.dart';
import '../player/player_view_model.dart';

class SearchView extends StatefulWidget {
  const SearchView({super.key});

  @override
  State<SearchView> createState() => _SearchViewState();
}

class _SearchViewState extends State<SearchView> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return PageScaffold(
      title: '在线搜索',
      subtitle: viewModel.onlineQuery.isEmpty
          ? '寻找歌曲、艺术家与专辑'
          : '${viewModel.onlineResults.length} 条结果 · ${viewModel.onlineSearchChannel.label}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SearchBar(
            controller: _controller,
            hintText: '输入歌曲、艺术家或专辑',
            leading: const Icon(Icons.search),
            trailing: [
              if (_controller.text.isNotEmpty)
                IconButton(
                  onPressed: () {
                    _controller.clear();
                    setState(() {});
                    viewModel.searchOnline('');
                  },
                  icon: const Icon(Icons.close),
                  tooltip: '清除',
                ),
              IconButton.filled(
                onPressed: viewModel.isSearchingOnline
                    ? null
                    : () => viewModel.searchOnline(_controller.text),
                icon: const Icon(Icons.arrow_forward),
                tooltip: '搜索',
              ),
            ],
            onChanged: (_) => setState(() {}),
            onSubmitted: viewModel.isSearchingOnline
                ? null
                : viewModel.searchOnline,
          ),
          const SizedBox(height: 14),
          _ChannelTabs(
            selected: viewModel.onlineSearchChannel,
            onSelected: viewModel.selectOnlineSearchChannel,
          ),
          const SizedBox(height: 24),
          AnimatedSwitcher(
            duration: const Duration(milliseconds: 280),
            child: _content(viewModel),
          ),
        ],
      ),
    );
  }

  Widget _content(PlayerViewModel viewModel) {
    if (viewModel.isSearchingOnline) {
      return const _SearchStatus(
        key: ValueKey('loading'),
        icon: Icons.graphic_eq,
        title: '正在寻找声音',
        loading: true,
      );
    }
    if (viewModel.onlineSearchError case final error?) {
      return _SearchStatus(
        key: const ValueKey('error'),
        icon: Icons.wifi_off,
        title: '暂时无法搜索',
        detail: error,
        action: FilledButton.icon(
          onPressed: () => viewModel.searchOnline(_controller.text),
          icon: const Icon(Icons.refresh),
          label: const Text('重试'),
        ),
      );
    }
    if (viewModel.onlineQuery.isEmpty) {
      return const _SearchStatus(
        key: ValueKey('idle'),
        icon: Icons.travel_explore,
        title: '从一次搜索开始',
      );
    }
    if (viewModel.onlineResults.isEmpty) {
      return const _SearchStatus(
        key: ValueKey('empty'),
        icon: Icons.search_off,
        title: '没有找到匹配歌曲',
      );
    }
    return Column(
      key: ValueKey('results-${viewModel.onlineQuery}'),
      children: [
        for (final track in viewModel.onlineResults)
          _OnlineTrackRow(track: track),
      ],
    );
  }
}

class _OnlineTrackRow extends StatelessWidget {
  const _OnlineTrackRow({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final active = viewModel.current?.id == track.id;
    return InkWell(
      onTap: () => viewModel.playTrack(track),
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
        child: Row(
          children: [
            _Artwork(track: track),
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
                    track.quality == null
                        ? track.artist
                        : '${track.artist} · ${track.quality}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
            if (MediaQuery.sizeOf(context).width > 650)
              Expanded(
                flex: 2,
                child: Text(
                  track.album,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            const SizedBox(width: 12),
            Text(_duration(track.duration)),
            const SizedBox(width: 8),
            Icon(
              active && viewModel.isPlaying
                  ? Icons.graphic_eq
                  : Icons.play_circle_outline,
              color: active ? Theme.of(context).colorScheme.primary : null,
            ),
          ],
        ),
      ),
    );
  }

  String _duration(Duration duration) => duration == Duration.zero
      ? '--:--'
      : '${duration.inMinutes}:${(duration.inSeconds % 60).toString().padLeft(2, '0')}';
}

class _ChannelTabs extends StatelessWidget {
  const _ChannelTabs({required this.selected, required this.onSelected});

  final OnlineSearchChannel selected;
  final ValueChanged<OnlineSearchChannel> onSelected;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          for (final channel in OnlineSearchChannel.values)
            Padding(
              padding: const EdgeInsets.only(right: 22),
              child: InkWell(
                onTap: () => onSelected(channel),
                borderRadius: BorderRadius.circular(4),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(2, 10, 2, 8),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        channel.label,
                        style: Theme.of(context).textTheme.titleMedium
                            ?.copyWith(
                              color: selected == channel
                                  ? colors.primary
                                  : colors.onSurfaceVariant,
                            ),
                      ),
                      const SizedBox(height: 6),
                      AnimatedContainer(
                        duration: const Duration(milliseconds: 180),
                        width: selected == channel ? 30 : 0,
                        height: 2,
                        color: colors.primary,
                      ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _Artwork extends StatelessWidget {
  const _Artwork({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final uri = track.artworkUri;
    if (uri == null) return OrganicArtwork(seed: track.id, size: 48);
    return ClipRRect(
      borderRadius: BorderRadius.circular(6),
      child: Image.network(
        uri,
        width: 48,
        height: 48,
        fit: BoxFit.cover,
        errorBuilder: (_, _, _) => OrganicArtwork(seed: track.id, size: 48),
      ),
    );
  }
}

class _SearchStatus extends StatelessWidget {
  const _SearchStatus({
    super.key,
    required this.icon,
    required this.title,
    this.detail,
    this.action,
    this.loading = false,
  });

  final IconData icon;
  final String title;
  final String? detail;
  final Widget? action;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 64),
      child: Column(
        children: [
          if (loading)
            const SizedBox.square(
              dimension: 38,
              child: CircularProgressIndicator(strokeWidth: 3),
            )
          else
            Icon(icon, size: 42, color: Theme.of(context).colorScheme.primary),
          const SizedBox(height: 16),
          Text(title, style: Theme.of(context).textTheme.headlineMedium),
          if (detail != null) ...[
            const SizedBox(height: 8),
            Text(detail!, textAlign: TextAlign.center),
          ],
          if (action != null) ...[const SizedBox(height: 20), action!],
        ],
      ),
    );
  }
}
