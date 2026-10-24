import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../state/player_view_model.dart';

/// 歌词面板。
///
/// 只监听当前行索引（每几秒变化一次），滚动用 [ScrollController.animateTo]；
/// 当前行的逐字进度是单独的 ValueNotifier，不会重建整段歌词。
class LyricsPane extends StatefulWidget {
  const LyricsPane({super.key, this.compact = false});

  final bool compact;

  @override
  State<LyricsPane> createState() => _LyricsPaneState();
}

class _LyricsPaneState extends State<LyricsPane> {
  final _controller = ScrollController();
  late final ValueNotifier<int> _indexNotifier;
  int _index = -1;
  double _extent = 58;

  @override
  void initState() {
    super.initState();
    _indexNotifier = context.read<PlayerViewModel>().lyricIndexListenable;
    _index = _indexNotifier.value;
    _indexNotifier.addListener(_onIndexChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _scrollTo(_index, animate: false);
    });
  }

  @override
  void dispose() {
    _indexNotifier.removeListener(_onIndexChanged);
    _controller.dispose();
    super.dispose();
  }

  void _onIndexChanged() {
    final index = _indexNotifier.value;
    if (index == _index) return;
    _index = index;
    if (mounted) setState(() {});
    _scrollTo(index, animate: true);
  }

  void _scrollTo(int index, {required bool animate}) {
    if (!_controller.hasClients || index < 0) return;
    final viewport = _controller.position.viewportDimension;
    final target = (index * _extent) - (viewport / 2) + (_extent / 2);
    final clamped = target.clamp(0.0, _controller.position.maxScrollExtent);
    if (animate && !MediaQuery.disableAnimationsOf(context)) {
      _controller.animateTo(
        clamped,
        duration: const Duration(milliseconds: 420),
        curve: Curves.easeOutCubic,
      );
    } else {
      _controller.jumpTo(clamped);
    }
  }

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final lyrics = model.lyrics;
    final track = model.current;
    if (track == null) return const SizedBox.shrink();
    if (lyrics.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              model.isLoadingLyrics
                  ? FLucideIcons.loader
                  : FLucideIcons.music2,
              size: 26,
              color: colors.mutedForeground,
            ),
            const SizedBox(height: 12),
            Text(
              model.isLoadingLyrics ? '正在加载歌词' : '暂无歌词',
              style: type.body.sm.copyWith(color: colors.mutedForeground),
            ),
            const SizedBox(height: 6),
            Text(
              track.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: type.display.xs.copyWith(
                color: colors.foreground.withValues(alpha: .85),
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
        ),
      );
    }
    _extent = widget.compact ? 48 : 58;
    return LayoutBuilder(
      builder: (context, constraints) {
        final padding = (constraints.maxHeight - _extent) / 2;
        return ListView.builder(
          controller: _controller,
          physics: const ClampingScrollPhysics(),
          itemExtent: _extent,
          padding: EdgeInsets.symmetric(vertical: padding.clamp(0, 2000)),
          itemCount: lyrics.length,
          itemBuilder: (context, index) => _LyricLine(
            index: index,
            text: lyrics[index].text,
            active: index == _index,
            compact: widget.compact,
          ),
        );
      },
    );
  }
}

class _LyricLine extends StatelessWidget {
  const _LyricLine({
    required this.index,
    required this.text,
    required this.active,
    required this.compact,
  });

  final int index;
  final String text;
  final bool active;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final baseSize = compact ? 17.0 : 22.0;
    return Align(
      alignment: Alignment.centerLeft,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          AnimatedDefaultTextStyle(
            duration: const Duration(milliseconds: 320),
            curve: Curves.easeOutCubic,
            style: type.body.md.copyWith(
              fontSize: active ? baseSize : baseSize * .84,
              height: 1.3,
              fontWeight: active ? FontWeight.w700 : FontWeight.w500,
              color: active
                  ? colors.primary
                  : colors.foreground.withValues(alpha: .34),
              letterSpacing: active ? -.2 : 0,
            ),
            child: Text(text, maxLines: 2, overflow: TextOverflow.ellipsis),
          ),
          if (active) ...[
            const SizedBox(height: 5),
            SizedBox(
              height: 2,
              width: compact ? 72 : 108,
              child: ValueListenableBuilder<double>(
                valueListenable: context
                    .read<PlayerViewModel>()
                    .lyricProgressListenable,
                builder: (context, progress, _) => Stack(
                  fit: StackFit.expand,
                  children: [
                    ColoredBox(
                      color: colors.primary.withValues(alpha: .18),
                    ),
                    FractionallySizedBox(
                      alignment: Alignment.centerLeft,
                      widthFactor: progress.clamp(0.0, 1.0),
                      child: ColoredBox(color: colors.primary),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
