import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../data/services/online_search_service.dart';
import '../../state/player_view_model.dart';
import '../widgets/common.dart';
import '../widgets/track_actions.dart';
import '../widgets/track_list.dart';

class SearchPage extends StatefulWidget {
  const SearchPage({super.key});

  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> {
  final _controller = TextEditingController();
  final _focusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    _controller.text = context.read<PlayerViewModel>().onlineQuery;
  }

  @override
  void dispose() {
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 在线搜索',
          title: '搜索音乐',
          subtitle: model.onlineQuery.isEmpty
              ? '跨平台寻找歌曲、艺术家与专辑'
              : '${model.onlineResults.length} 条结果 · ${model.onlineSearchChannel.label} · 第 ${model.onlineSearchPage} 页',
          actions: [
            if (model.onlineResults.isNotEmpty)
              FButton(
                variant: FButtonVariant.outline,
                size: FButtonSizeVariant.sm,
                mainAxisSize: MainAxisSize.min,
                prefix: const Icon(FLucideIcons.listFilter, size: 15),
                onPress: () => model.searchOnline(_controller.text),
                child: const Text('重新搜索'),
              ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: PageBody(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: FTextField(
                        control: FTextFieldControl.managed(
                          controller: _controller,
                        ),
                        focusNode: _focusNode,
                        hint: '输入歌曲、艺术家或专辑',
                        clearable: (_) => true,
                        prefixBuilder: (context, style, _) =>
                            Icon(FLucideIcons.search, size: 16),
                        onSubmit: model.isSearchingOnline
                            ? null
                            : (value) => model.searchOnline(value),
                      ),
                    ),
                    const SizedBox(width: 10),
                    FButton(
                      size: FButtonSizeVariant.md,
                      mainAxisSize: MainAxisSize.min,
                      prefix: const Icon(FLucideIcons.arrowRight, size: 16),
                      onPress: model.isSearchingOnline
                          ? null
                          : () => model.searchOnline(_controller.text),
                      child: const Text('搜索'),
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: Row(
                    children: [
                      for (final channel in OnlineSearchChannel.values)
                        Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: FilterPill(
                            label: channel.label,
                            selected: model.onlineSearchChannel == channel,
                            onPress: () => model.selectOnlineSearchChannel(
                              channel,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 10),
        Expanded(
          child: PageBody(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 240),
              switchInCurve: Curves.easeOutCubic,
              switchOutCurve: Curves.easeInCubic,
              child: _content(model, colors, type),
            ),
          ),
        ),
      ],
    );
  }

  Widget _content(PlayerViewModel model, FColors colors, FTypography type) {
    if (model.isSearchingOnline) {
      return const LoadingState(key: ValueKey('loading'), label: '正在寻找声音');
    }
    if (model.onlineSearchError case final error?) {
      return ErrorState(
        key: const ValueKey('error'),
        message: error,
        onRetry: () => model.searchOnline(_controller.text),
      );
    }
    if (model.onlineQuery.isEmpty) {
      return EmptyState(
        key: const ValueKey('idle'),
        icon: FLucideIcons.compass,
        title: '从一次搜索开始',
        description: '输入歌名、艺术家或专辑，WCMusic 会同时向多个平台查询。',
        action: FButton(
          variant: FButtonVariant.outline,
          size: FButtonSizeVariant.sm,
          mainAxisSize: MainAxisSize.min,
          prefix: const Icon(FLucideIcons.search, size: 15),
          onPress: _focusNode.requestFocus,
          child: const Text('聚焦搜索框'),
        ),
      );
    }
    if (model.onlineResults.isEmpty) {
      return EmptyState(
        key: ValueKey('empty-${model.onlineSearchPage}'),
        icon: FLucideIcons.searchX,
        title: model.onlineSearchPage > 1 ? '这一页没有更多歌曲' : '没有找到匹配歌曲',
        description: '换一个关键词，或者切换平台再试一次。',
        action: model.onlineSearchPage > 1
            ? FButton(
                variant: FButtonVariant.outline,
                size: FButtonSizeVariant.sm,
                mainAxisSize: MainAxisSize.min,
                onPress: () => model.searchOnline(
                  model.onlineQuery,
                  page: model.onlineSearchPage - 1,
                ),
                child: const Text('上一页'),
              )
            : null,
      );
    }
    return Column(
      key: ValueKey('results-${model.onlineQuery}-${model.onlineSearchPage}'),
      children: [
        Expanded(
          child: TrackListView(
            tracks: model.onlineResults,
            currentId: model.current?.id,
            isPlaying: model.isPlaying,
            onPlay: model.playTrack,
            onMenu: (track) => showTrackActions(context, track),
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(0, 6, 0, 18),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              FButton(
                variant: FButtonVariant.outline,
                size: FButtonSizeVariant.sm,
                mainAxisSize: MainAxisSize.min,
                prefix: const Icon(FLucideIcons.chevronLeft, size: 15),
                onPress: model.onlineSearchPage <= 1
                    ? null
                    : () => model.searchOnline(
                        model.onlineQuery,
                        page: model.onlineSearchPage - 1,
                      ),
                child: const Text('上一页'),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Text(
                  '第 ${model.onlineSearchPage} 页',
                  style: type.body.xs.copyWith(color: colors.mutedForeground),
                ),
              ),
              FButton(
                variant: FButtonVariant.outline,
                size: FButtonSizeVariant.sm,
                mainAxisSize: MainAxisSize.min,
                suffix: const Icon(FLucideIcons.chevronRight, size: 15),
                onPress: !model.canLoadMoreOnlineResults
                    ? null
                    : () => model.searchOnline(
                        model.onlineQuery,
                        page: model.onlineSearchPage + 1,
                      ),
                child: const Text('下一页'),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
