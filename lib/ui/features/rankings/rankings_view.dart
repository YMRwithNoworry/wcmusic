import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../data/services/online_search_service.dart';
import '../../../domain/models/track.dart';
import '../../core/animated_list_item.dart';
import '../../core/organic_artwork.dart';
import '../../core/page_scaffold.dart';
import '../../core/remote_artwork.dart';
import '../../core/track_favorite_menu.dart';
import '../player/player_view_model.dart';

class RankingsView extends StatefulWidget {
  const RankingsView({super.key});

  @override
  State<RankingsView> createState() => _RankingsViewState();
}

class _RankingsViewState extends State<RankingsView> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final viewModel = context.read<PlayerViewModel>();
      if (viewModel.rankings.isEmpty && !viewModel.isLoadingRankings) {
        viewModel.loadRankings(viewModel.rankingChannel);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return PageScaffold(
      title: '排行榜',
      subtitle: viewModel.selectedRanking == null
          ? '查看各音乐平台的实时榜单'
          : '${viewModel.rankingChannel.label} · ${viewModel.selectedRanking!.name}',
      actions: [
        IconButton.filledTonal(
          onPressed: viewModel.isLoadingRankings
              ? null
              : () => viewModel.loadRankings(viewModel.rankingChannel),
          icon: const Icon(Icons.refresh),
          tooltip: '刷新排行榜',
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _PlatformTabs(viewModel: viewModel),
          const SizedBox(height: 20),
          if (viewModel.isLoadingRankings && viewModel.rankings.isEmpty)
            const _LoadingState(label: '正在载入榜单')
          else if (viewModel.rankings.isEmpty)
            _ErrorState(
              message: viewModel.rankingsError ?? '该平台暂时没有可用榜单',
              onRetry: () => viewModel.loadRankings(viewModel.rankingChannel),
            )
          else
            LayoutBuilder(
              builder: (context, constraints) {
                if (constraints.maxWidth < 720) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      _RankingDropdown(viewModel: viewModel),
                      const SizedBox(height: 18),
                      _RankingTracks(viewModel: viewModel),
                    ],
                  );
                }
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 240,
                      child: _RankingList(viewModel: viewModel),
                    ),
                    const SizedBox(width: 28),
                    Expanded(child: _RankingTracks(viewModel: viewModel)),
                  ],
                );
              },
            ),
        ],
      ),
    );
  }
}

class _PlatformTabs extends StatelessWidget {
  const _PlatformTabs({required this.viewModel});

  final PlayerViewModel viewModel;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: SegmentedButton<OnlineSearchChannel>(
        segments: [
          for (final channel in OnlineSearchChannel.values)
            ButtonSegment(value: channel, label: Text(channel.label)),
        ],
        selected: {viewModel.rankingChannel},
        onSelectionChanged: viewModel.isLoadingRankings
            ? null
            : (value) => viewModel.loadRankings(value.single),
      ),
    );
  }
}

class _RankingList extends StatelessWidget {
  const _RankingList({required this.viewModel});

  final PlayerViewModel viewModel;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (final ranking in viewModel.rankings)
          AnimatedListItem(
            index: viewModel.rankings.indexOf(ranking),
            child: Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: ListTile(
                selected: ranking.id == viewModel.selectedRanking?.id,
                selectedTileColor: Theme.of(
                  context,
                ).colorScheme.secondaryContainer,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8),
                ),
                leading: _RankingArtwork(ranking: ranking, size: 42),
                title: Text(
                  ranking.name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                onTap: () => viewModel.selectRanking(ranking),
              ),
            ),
          ),
      ],
    );
  }
}

class _RankingDropdown extends StatelessWidget {
  const _RankingDropdown({required this.viewModel});

  final PlayerViewModel viewModel;

  @override
  Widget build(BuildContext context) {
    return DropdownButtonFormField<String>(
      initialValue: viewModel.selectedRanking?.id,
      decoration: const InputDecoration(
        labelText: '选择榜单',
        prefixIcon: Icon(Icons.leaderboard_outlined),
      ),
      items: [
        for (final ranking in viewModel.rankings)
          DropdownMenuItem(value: ranking.id, child: Text(ranking.name)),
      ],
      onChanged: (id) {
        if (id == null) return;
        viewModel.selectRanking(
          viewModel.rankings.firstWhere((ranking) => ranking.id == id),
        );
      },
    );
  }
}

class _RankingTracks extends StatelessWidget {
  const _RankingTracks({required this.viewModel});

  final PlayerViewModel viewModel;

  @override
  Widget build(BuildContext context) {
    if (viewModel.isLoadingRankingTracks) {
      return const _LoadingState(label: '正在载入榜单歌曲');
    }
    if (viewModel.rankingTracks.isEmpty) {
      return _ErrorState(
        message: viewModel.rankingsError ?? '该榜单暂时没有歌曲',
        onRetry: viewModel.selectedRanking == null
            ? null
            : () => viewModel.selectRanking(viewModel.selectedRanking!),
      );
    }
    return Column(
      children: [
        for (var i = 0; i < viewModel.rankingTracks.length; i++)
          AnimatedListItem(
            index: i,
            delay: const Duration(milliseconds: 50),
            child: _RankingTrackRow(
              rank: i + 1,
              track: viewModel.rankingTracks[i],
            ),
          ),
      ],
    );
  }
}

class _RankingTrackRow extends StatelessWidget {
  const _RankingTrackRow({required this.rank, required this.track});

  final int rank;
  final Track track;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final active = viewModel.current?.id == track.id;
    return HoverScaleCard(
      scaleAmount: 0.015,
      onTap: () => viewModel.playTrack(track),
      child: InkWell(
        onTap: () => viewModel.playTrack(track),
        onSecondaryTap: () => showTrackFavoriteMenu(context, track),
        onLongPress: () => showTrackFavoriteMenu(context, track),
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
          child: Row(
            children: [
              SizedBox(
                width: 34,
                child: Text(
                  '$rank',
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    color: rank <= 3
                        ? Theme.of(context).colorScheme.primary
                        : null,
                    fontWeight: rank <= 3 ? FontWeight.w700 : null,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              _TrackArtwork(track: track),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      track.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    Text(
                      '${track.artist} · ${track.album}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
              IconButton(
                onPressed: () => showTrackFavoriteMenu(context, track),
                icon: Icon(
                  active && viewModel.isPlaying
                      ? Icons.graphic_eq
                      : Icons.more_horiz,
                ),
                tooltip: '更多',
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _RankingArtwork extends StatelessWidget {
  const _RankingArtwork({required this.ranking, required this.size});

  final PlatformRanking ranking;
  final double size;

  @override
  Widget build(BuildContext context) {
    final uri = ranking.artworkUri;
    if (uri == null || uri.isEmpty) {
      return OrganicArtwork(seed: ranking.id, size: size);
    }
    return SizedBox.square(
      dimension: size,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(6),
        child: RemoteArtwork(url: uri, seed: ranking.id),
      ),
    );
  }
}

class _TrackArtwork extends StatelessWidget {
  const _TrackArtwork({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final uri = track.artworkUri;
    if (uri == null || uri.isEmpty) {
      return OrganicArtwork(seed: track.id, size: 46);
    }
    return SizedBox.square(
      dimension: 46,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(6),
        child: RemoteArtwork(url: uri, seed: track.id),
      ),
    );
  }
}

class _LoadingState extends StatelessWidget {
  const _LoadingState({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 56),
      child: Column(
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 14),
          Text(label),
        ],
      ),
    );
  }
}

class _ErrorState extends StatelessWidget {
  const _ErrorState({required this.message, this.onRetry});

  final String message;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48),
      child: Column(
        children: [
          const Icon(Icons.leaderboard_outlined, size: 42),
          const SizedBox(height: 12),
          Text(message, textAlign: TextAlign.center),
          if (onRetry != null) ...[
            const SizedBox(height: 18),
            FilledButton.icon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: const Text('重试'),
            ),
          ],
        ],
      ),
    );
  }
}
