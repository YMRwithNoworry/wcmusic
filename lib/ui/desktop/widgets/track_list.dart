import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';

import '../../../domain/models/track.dart';
import '../widgets/artwork.dart';

/// 三根跳动的音量条，表示正在播放。
class PlayingBars extends StatefulWidget {
  const PlayingBars({
    super.key,
    required this.playing,
    this.color,
    this.height = 14,
  });

  final bool playing;
  final Color? color;
  final double height;

  @override
  State<PlayingBars> createState() => _PlayingBarsState();
}

class _PlayingBarsState extends State<PlayingBars>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 900),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sync();
  }

  @override
  void didUpdateWidget(PlayingBars oldWidget) {
    super.didUpdateWidget(oldWidget);
    _sync();
  }

  void _sync() {
    if (!widget.playing || MediaQuery.disableAnimationsOf(context)) {
      _controller
        ..stop()
        ..value = 0;
    } else if (!_controller.isAnimating) {
      _controller.repeat();
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = widget.color ?? context.theme.colors.primary;
    if (!widget.playing) {
      return SizedBox(
        width: 16,
        height: widget.height,
        child: Icon(
          FLucideIcons.pause,
          size: 14,
          color: context.theme.colors.mutedForeground,
        ),
      );
    }
    return SizedBox(
      width: 16,
      height: widget.height,
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) => CustomPaint(
          painter: _BarsPainter(_controller.value, color),
        ),
      ),
    );
  }
}

class _BarsPainter extends CustomPainter {
  _BarsPainter(this.phase, this.color);

  final double phase;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..strokeCap = StrokeCap.round
      ..strokeWidth = 2.4;
    for (var index = 0; index < 3; index++) {
      final phase = this.phase * math.pi * 2 + index * 1.7;
      final wave = .35 + .65 * (0.5 + 0.5 * math.sin(phase));
      final height = size.height * wave;
      final x = 2.5 + index * 5.5;
      canvas.drawLine(
        Offset(x, (size.height + height) / 2),
        Offset(x, (size.height - height) / 2),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _BarsPainter oldDelegate) =>
      oldDelegate.phase != phase || oldDelegate.color != color;
}

/// 曲目行。
class TrackRow extends StatefulWidget {
  const TrackRow({
    super.key,
    required this.track,
    required this.onPlay,
    this.onMenu,
    this.active = false,
    this.playing = false,
    this.rank,
    this.showAlbum = true,
    this.showArtwork = true,
    this.indexLabel,
  });

  final Track track;
  final VoidCallback onPlay;
  final VoidCallback? onMenu;
  final bool active;
  final bool playing;
  final int? rank;
  final bool showAlbum;
  final bool showArtwork;
  final String? indexLabel;

  @override
  State<TrackRow> createState() => _TrackRowState();
}

class _TrackRowState extends State<TrackRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final active = widget.active;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onPlay,
        onSecondaryTap: widget.onMenu,
        onLongPress: widget.onMenu,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          curve: Curves.easeOut,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: BoxDecoration(
            color: _hovered
                ? colors.secondary.withValues(alpha: .72)
                : colors.background.withValues(alpha: 0),
            borderRadius: const BorderRadius.all(Radius.circular(12)),
          ),
          child: Row(
            children: [
              if (widget.rank != null) ...[
                SizedBox(
                  width: 30,
                  child: Text(
                    '${widget.rank}',
                    textAlign: TextAlign.center,
                    style: type.body.sm.copyWith(
                      color: widget.rank! <= 3
                          ? colors.primary
                          : colors.mutedForeground,
                      fontWeight: widget.rank! <= 3
                          ? FontWeight.w700
                          : FontWeight.w500,
                      fontFeatures: const [FontFeature.tabularFigures()],
                    ),
                  ),
                ),
                const SizedBox(width: 6),
              ] else if (widget.indexLabel != null) ...[
                SizedBox(
                  width: 26,
                  child: Text(
                    widget.indexLabel!,
                    textAlign: TextAlign.center,
                    style: type.body.xs.copyWith(
                      color: active ? colors.primary : colors.mutedForeground,
                      fontFeatures: const [FontFeature.tabularFigures()],
                    ),
                  ),
                ),
                const SizedBox(width: 8),
              ],
              if (widget.showArtwork) ...[
                ArtworkView(
                  seed: widget.track.id,
                  url: widget.track.artworkUri,
                  size: 42,
                  radius: 8,
                ),
                const SizedBox(width: 14),
              ],
              Expanded(
                flex: 5,
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            widget.track.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: type.body.sm.copyWith(
                              color: active
                                  ? colors.primary
                                  : colors.foreground,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                        if (active) ...[
                          const SizedBox(width: 8),
                          PlayingBars(playing: widget.playing),
                        ],
                      ],
                    ),
                    const SizedBox(height: 2),
                    Text(
                      widget.track.quality == null
                          ? widget.track.artist
                          : '${widget.track.artist} · ${widget.track.quality}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                      ),
                    ),
                  ],
                ),
              ),
              if (widget.showAlbum)
                Expanded(
                  flex: 3,
                  child: Text(
                    widget.track.album,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: type.body.xs.copyWith(
                      color: colors.mutedForeground,
                    ),
                  ),
                ),
              const SizedBox(width: 10),
              Text(
                widget.track.duration <= Duration.zero
                    ? '--:--'
                    : formatDuration(widget.track.duration),
                style: type.body.xs.copyWith(
                  color: colors.mutedForeground,
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
              const SizedBox(width: 4),
              SizedBox(
                width: 34,
                height: 34,
                child: _hovered || widget.onMenu != null
                    ? FTooltip(
                        tipBuilder: (context, _) => const Text('更多操作'),
                        child: FButton.icon(
                          variant: FButtonVariant.ghost,
                          size: FButtonSizeVariant.xs,
                          onPress: widget.onMenu,
                          child: const Icon(FLucideIcons.moreHorizontal, size: 16),
                        ),
                      )
                    : const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

String formatDuration(Duration duration) {
  // 0:00 是合法的播放位置；只有负值或未知时长才显示占位符。
  if (duration.isNegative) return '--:--';
  final minutes = duration.inMinutes;
  final seconds = duration.inSeconds % 60;
  return '$minutes:${seconds.toString().padLeft(2, '0')}';
}

/// 固定行高的曲目列表，滚动时布局成本稳定。
class TrackListView extends StatelessWidget {
  const TrackListView({
    super.key,
    required this.tracks,
    required this.onPlay,
    this.onMenu,
    this.currentId,
    this.isPlaying = false,
    this.padding = const EdgeInsets.fromLTRB(20, 4, 20, 24),
    this.showAlbum = true,
    this.showArtwork = true,
    this.ranked = false,
  });

  final List<Track> tracks;
  final void Function(Track track) onPlay;
  final void Function(Track track)? onMenu;
  final String? currentId;
  final bool isPlaying;
  final EdgeInsets padding;
  final bool showAlbum;
  final bool showArtwork;
  final bool ranked;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      padding: padding,
      itemExtent: 62,
      itemCount: tracks.length,
      itemBuilder: (context, index) {
        final track = tracks[index];
        return TrackRow(
          track: track,
          rank: ranked ? index + 1 : null,
          indexLabel: ranked
              ? null
              : (index + 1).toString().padLeft(2, '0'),
          active: currentId == track.id,
          playing: isPlaying,
          showAlbum: showAlbum,
          showArtwork: showArtwork,
          onPlay: () => onPlay(track),
          onMenu: onMenu == null ? null : () => onMenu!(track),
        );
      },
    );
  }
}
