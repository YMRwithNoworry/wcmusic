import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../state/player_view_model.dart';

/// 歌词面板。
///
/// 只监听当前行索引（每几秒变化一次），滚动用 [ScrollController.animateTo]；
/// 当前行的卡拉 OK 填充是每行自带的 [KaraokeText]，它单独监听
/// `lyricProgressListenable`，进度刷新不会重建整段歌词。
class LyricsPane extends StatefulWidget {
  const LyricsPane({super.key, this.compact = false, this.maxLines = 1});

  final bool compact;

  /// 每行歌词最多显示几行；翻译存在时原文始终只占一行。
  final int maxLines;

  @override
  State<LyricsPane> createState() => _LyricsPaneState();
}

class _LyricsPaneState extends State<LyricsPane> {
  final _controller = ScrollController();
  late final ValueNotifier<int> _indexNotifier;
  late final ValueNotifier<double> _progressNotifier;
  int _index = -1;
  double _extent = 66;

  @override
  void initState() {
    super.initState();
    final model = context.read<PlayerViewModel>();
    _indexNotifier = model.lyricIndexListenable;
    _progressNotifier = model.lyricProgressListenable;
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
    _extent = widget.compact ? 52 : 66;
    return LayoutBuilder(
      builder: (context, constraints) {
        final padding = (constraints.maxHeight - _extent) / 2;
        return ListView.builder(
          controller: _controller,
          physics: const ClampingScrollPhysics(),
          itemExtent: _extent,
          padding: EdgeInsets.symmetric(vertical: padding.clamp(0, 2000)),
          itemCount: lyrics.length,
          itemBuilder: (context, index) {
            final translation = lyrics[index].translation;
            return _LyricLine(
              text: lyrics[index].text,
              translation: translation,
              active: index == _index,
              compact: widget.compact,
              progress: _progressNotifier,
              // 有翻译副行时原文必须单行，否则 itemExtent 会被撑破。
              maxLines: translation == null || translation == lyrics[index].text
                  ? widget.maxLines
                  : 1,
            );
          },
        );
      },
    );
  }
}

class _LyricLine extends StatelessWidget {
  const _LyricLine({
    required this.text,
    required this.translation,
    required this.active,
    required this.compact,
    required this.progress,
    required this.maxLines,
  });

  final String text;
  final String? translation;
  final bool active;
  final bool compact;
  final ValueListenable<double> progress;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final baseSize = compact ? 17.0 : 22.0;
    final size = active ? baseSize : baseSize * .84;
    final translation = this.translation;
    final filled = colors.primary;
    final unfilled = active
        ? colors.mutedForeground
        : colors.foreground.withValues(alpha: .34);
    final style = type.body.md.copyWith(
      fontSize: size,
      height: 1.3,
      fontWeight: active ? FontWeight.w700 : FontWeight.w500,
      letterSpacing: active ? -.2 : 0,
    );
    return Align(
      alignment: Alignment.centerLeft,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // 当前行做卡拉 OK 填充：未唱部分灰色，已唱部分用主题色从左往右
          // 覆盖过去，替代原来行下方的小进度条。
          if (active)
            KaraokeText(
              text: text,
              progress: progress,
              style: style,
              filled: filled,
              unfilled: unfilled,
              maxLines: maxLines,
            )
          else
            Text(
              text,
              maxLines: maxLines,
              overflow: TextOverflow.ellipsis,
              style: style.copyWith(color: unfilled),
            ),
          if (translation != null && translation != text)
            Text(
              translation,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: type.body.sm.copyWith(
                fontSize: size * .62,
                height: 1.4,
                color: active
                    ? colors.foreground.withValues(alpha: .82)
                    : colors.foreground.withValues(alpha: .24),
              ),
            ),
        ],
      ),
    );
  }
}

/// 卡拉 OK 文字：按 [progress] 从左到右把 [filled] 色刷到 [unfilled] 色的文字上。
///
/// 文字先排一次版拿到逐字位置，再按「字宽百分比」切开宽度，让每个字都在自己
/// 的播放时段内被填满；进度只通过 [ValueListenableBuilder] 刷新当前行，不触发
/// 外层列表重建。
class KaraokeText extends StatefulWidget {
  const KaraokeText({
    super.key,
    required this.text,
    required this.progress,
    required this.style,
    required this.filled,
    required this.unfilled,
    this.maxLines = 1,
  });

  final String text;
  final ValueListenable<double> progress;
  final TextStyle style;
  final Color filled;
  final Color unfilled;
  final int maxLines;

  @override
  State<KaraokeText> createState() => _KaraokeTextState();
}

class _KaraokeTextState extends State<KaraokeText> {
  /// 每个字在整行文字里的起止比例（按字宽切分，比按字数切分更贴节奏）。
  List<({double start, double end})> _slices = const [];

  /// 上一次排版用的宽度；为 null 表示还没拿到约束。
  double? _laidOutWidth;

  @override
  void initState() {
    super.initState();
    _layout(double.infinity);
  }

  @override
  void didUpdateWidget(KaraokeText oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.text != widget.text ||
        oldWidget.style != widget.style ||
        oldWidget.maxLines != widget.maxLines) {
      _layout(_laidOutWidth ?? double.infinity);
    }
  }

  /// 按 [width] 排版一次，算出逐字比例。
  ///
  /// 行内是 [Stack] 自动取最宽子项，所以必须用外层给出的宽度排版，单行才不会被
  /// 截断；Infinity 表示还在测量阶段，按单行自然宽度处理。
  void _layout(double width) {
    _laidOutWidth = width;
    final painter = TextPainter(
      text: TextSpan(text: widget.text, style: widget.style),
      textDirection: TextDirection.ltr,
      maxLines: widget.maxLines,
    );
    painter.layout(maxWidth: width);
    final lineWidth = painter.width;
    if (lineWidth <= 0) {
      _slices = const [];
      painter.dispose();
      return;
    }
    final slices = <({double start, double end})>[];
    var previous = 0.0;
    for (var i = 0; i < widget.text.length; i++) {
      final extent = painter
          .getBoxesForSelection(
            TextSelection(baseOffset: i, extentOffset: i + 1),
          )
          .fold<double>(0, (sum, box) => sum + box.right);
      // 组合字符 / 空白的框宽可能相同，保证进度单调不减即可。
      final end = extent <= previous ? previous : extent;
      slices.add((start: previous / lineWidth, end: end / lineWidth));
      previous = end;
    }
    // 最后一个字右边界可能因字距略小于排版宽度，补到 1 保证整行都能刷满。
    if (slices.isNotEmpty) {
      final last = slices.removeLast();
      slices.add((start: last.start, end: 1));
    }
    painter.dispose();
    _slices = slices;
  }

  @override
  Widget build(BuildContext context) {
    final style = DefaultTextStyle.of(context).style.merge(widget.style);
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth != _laidOutWidth) {
          _layout(constraints.maxWidth);
        }
        return ValueListenableBuilder<double>(
          valueListenable: widget.progress,
          builder: (context, progress, _) {
            // 当前字内的填充比例，让高亮在字与字之间连续推进。
            final fraction = _filledFraction(progress);
            return Stack(
              children: [
                Text(
                  widget.text,
                  maxLines: widget.maxLines,
                  overflow: TextOverflow.ellipsis,
                  style: style.copyWith(color: widget.unfilled),
                ),
                ClipRect(
                  clipper: _FillClipper(fraction),
                  child: Text(
                    widget.text,
                    maxLines: widget.maxLines,
                    overflow: TextOverflow.ellipsis,
                    style: style.copyWith(color: widget.filled),
                  ),
                ),
              ],
            );
          },
        );
      },
    );
  }

  double _filledFraction(double progress) {
    var filled = progress.clamp(0.0, 1.0);
    for (final slice in _slices) {
      if (progress < slice.end) {
        final span = slice.end - slice.start;
        if (span > 0) {
          final inside = ((progress - slice.start) / span).clamp(0.0, 1.0);
          filled = slice.start + span * inside;
        }
        break;
      }
    }
    return filled;
  }
}

class _FillClipper extends CustomClipper<Rect> {
  const _FillClipper(this.fraction);

  /// 0-1，横向已经「唱到」的比例。
  final double fraction;

  @override
  Rect getClip(Size size) =>
      Rect.fromLTWH(0, 0, size.width * fraction, size.height);

  @override
  bool shouldReclip(_FillClipper oldClipper) => oldClipper.fraction != fraction;
}
