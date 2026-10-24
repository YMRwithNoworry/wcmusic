import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../data/services/online_search_service.dart';
import '../../state/player_view_model.dart';
import '../widgets/artwork.dart';
import '../widgets/common.dart';
import '../widgets/track_actions.dart';
import '../widgets/track_list.dart';

class RankingsPage extends StatefulWidget {
  const RankingsPage({super.key});

  @override
  State<RankingsPage> createState() => _RankingsPageState();
}

class _RankingsPageState extends State<RankingsPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final model = context.read<PlayerViewModel>();
      if (model.rankings.isEmpty && !model.isLoadingRankings) {
        model.loadRankings(model.rankingChannel);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final colors = context.theme.colors;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 排行榜',
          title: '排行榜',
          subtitle: model.selectedRanking == null
              ? '查看各音乐平台的实时榜单'
              : '${model.rankingChannel.label} · ${model.selectedRanking!.name}',
          actions: [
            FButton(
              variant: FButtonVariant.outline,
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.refreshCw, size: 15),
              onPress: model.isLoadingRankings
                  ? null
                  : () => model.loadRankings(model.rankingChannel),
              child: const Text('刷新'),
            ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: PageBody(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  for (final channel in OnlineSearchChannel.values)
                    Padding(
                      padding: const EdgeInsets.only(right: 8),
                      child: FilterPill(
                        label: channel.label,
                        selected: model.rankingChannel == channel,
                        onPress: model.isLoadingRankings
                            ? () {}
                            : () => model.loadRankings(channel),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: 16),
        Expanded(
          child: PageBody(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 240),
              switchInCurve: Curves.easeOutCubic,
              child: _body(model, colors),
            ),
          ),
        ),
      ],
    );
  }

  Widget _body(PlayerViewModel model, FColors colors) {
    if (model.isLoadingRankings && model.rankings.isEmpty) {
      return const LoadingState(
        key: ValueKey('loading'),
        label: '正在载入榜单',
      );
    }
    if (model.rankings.isEmpty) {
      return ErrorState(
        key: const ValueKey('error'),
        message: model.rankingsError ?? '该平台暂时没有可用榜单',
        onRetry: () => model.loadRankings(model.rankingChannel),
      );
    }
    return LayoutBuilder(
      key: const ValueKey('content'),
      builder: (context, constraints) {
        final wide = constraints.maxWidth >= 860;
        if (!wide) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 32),
                child: FSelect<String>(
                  control: FSelectControl.managed(
                    initial: model.selectedRanking?.id,
                    onChange: (id) {
                      if (id == null) return;
                      final ranking = model.rankings.firstWhere(
                        (item) => item.id == id,
                      );
                      model.selectRanking(ranking);
                    },
                  ),
                  label: const Text('选择榜单'),
                  items: {
                    for (final ranking in model.rankings)
                      ranking.name: ranking.id,
                  },
                ),
              ),
              const SizedBox(height: 12),
              Expanded(child: _tracks(model)),
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SizedBox(
              width: 288,
              child: ListView.builder(
                padding: const EdgeInsets.fromLTRB(32, 0, 10, 20),
                itemCount: model.rankings.length,
                itemBuilder: (context, index) {
                  final ranking = model.rankings[index];
                  return Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _RankingTile(
                      ranking: ranking,
                      selected: ranking.id == model.selectedRanking?.id,
                      onTap: () => model.selectRanking(ranking),
                    ),
                  );
                },
              ),
            ),
            Expanded(child: _tracks(model)),
          ],
        );
      },
    );
  }

  Widget _tracks(PlayerViewModel model) {
    if (model.isLoadingRankingTracks) {
      return const LoadingState(label: '正在载入榜单歌曲');
    }
    if (model.rankingTracks.isEmpty) {
      return ErrorState(
        message: model.rankingsError ?? '该榜单暂时没有歌曲',
        onRetry: model.selectedRanking == null
            ? null
            : () => model.selectRanking(model.selectedRanking!),
      );
    }
    return TrackListView(
      tracks: model.rankingTracks,
      ranked: true,
      currentId: model.current?.id,
      isPlaying: model.isPlaying,
      onPlay: model.playTrack,
      onMenu: (track) => showTrackActions(context, track),
      padding: const EdgeInsets.fromLTRB(22, 0, 32, 20),
    );
  }
}

class _RankingTile extends StatefulWidget {
  const _RankingTile({
    required this.ranking,
    required this.selected,
    required this.onTap,
  });

  final PlatformRanking ranking;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_RankingTile> createState() => _RankingTileState();
}

class _RankingTileState extends State<_RankingTile> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 170),
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: widget.selected
                ? colors.primary.withValues(alpha: .12)
                : _hovered
                ? colors.secondary.withValues(alpha: .7)
                : const Color(0x00000000),
            borderRadius: const BorderRadius.all(Radius.circular(12)),
            border: Border.all(
              color: widget.selected
                  ? colors.primary.withValues(alpha: .4)
                  : const Color(0x00000000),
            ),
          ),
          child: Row(
            children: [
              ArtworkView(
                seed: widget.ranking.id,
                url: widget.ranking.artworkUri,
                size: 40,
                radius: 9,
              ),
              const SizedBox(width: 11),
              Expanded(
                child: Text(
                  widget.ranking.name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: type.body.xs.copyWith(
                    color: widget.selected
                        ? colors.primary
                        : colors.foreground,
                    fontWeight: widget.selected
                        ? FontWeight.w600
                        : FontWeight.w500,
                    height: 1.35,
                  ),
                ),
              ),
              if (widget.selected)
                Icon(
                  FLucideIcons.chevronRight,
                  size: 14,
                  color: colors.primary,
                ),
            ],
          ),
        ),
      ),
    );
  }
}
