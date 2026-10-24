import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/playback_mode.dart';
import '../../state/player_view_model.dart';
import '../widgets/track_list.dart';

/// 平滑进度条。
///
/// 播放器每 100~500ms 才推一次进度，这里用短动画把两次推送之间补平，视觉上
/// 是连续移动的；拖动时暂停插值并直接预览目标位置。
class SeekBar extends StatefulWidget {
  const SeekBar({super.key, required this.compact, this.showTimes = true});

  final bool compact;
  final bool showTimes;

  @override
  State<SeekBar> createState() => _SeekBarState();
}

class _SeekBarState extends State<SeekBar> with SingleTickerProviderStateMixin {
  late final AnimationController _ticker = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 320),
  );

  double _from = 0;
  double _to = 0;
  double? _dragFraction;
  bool _hovered = false;

  late final PlayerViewModel _model;

  @override
  void initState() {
    super.initState();
    // dispose 阶段不能再向上查找祖先，所以在这里一次性缓存。
    _model = context.read<PlayerViewModel>();
    _from = _to = _fraction(_model.position, _model.playbackDuration);
    _model.positionListenable.addListener(_onPosition);
    _model.durationListenable.addListener(_onPosition);
  }

  @override
  void dispose() {
    _model.positionListenable.removeListener(_onPosition);
    _model.durationListenable.removeListener(_onPosition);
    _ticker.dispose();
    super.dispose();
  }

  double _fraction(Duration position, Duration duration) {
    final total = duration.inMilliseconds;
    if (total <= 0) return 0;
    return (position.inMilliseconds / total).clamp(0.0, 1.0);
  }

  double get _displayed => _dragFraction ?? _current;

  double get _current => _from + (_to - _from) * _ticker.value;

  void _onPosition() {
    if (!mounted) return;
    final model = _model;
    final target = _fraction(model.position, model.playbackDuration);
    if (_dragFraction != null || !model.isPlaying) {
      setState(() {
        _from = _to = target;
      });
      _ticker.value = 0;
      return;
    }
    final start = _current;
    if ((target - start).abs() < .0005) return;
    setState(() {
      _from = start;
      _to = target;
    });
    _ticker.forward(from: 0);
  }

  void _seekTo(double fraction) {
    final model = _model;
    final total = model.playbackDuration.inMilliseconds;
    if (total <= 0) return;
    model.seek(fraction * total);
  }

  void _updateDrag(Offset localPosition, double width) {
    if (width <= 0) return;
    setState(() {
      _dragFraction = (localPosition.dx / width).clamp(0.0, 1.0);
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final model = context.watch<PlayerViewModel>();
    final duration = model.playbackDuration;
    final hasDuration = duration > Duration.zero;
    final height = widget.compact ? 22.0 : 30.0;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        MouseRegion(
          cursor: hasDuration
              ? SystemMouseCursors.click
              : MouseCursor.defer,
          onEnter: (_) => setState(() => _hovered = true),
          onExit: (_) => setState(() => _hovered = false),
          child: LayoutBuilder(
            builder: (context, constraints) => GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTapDown: hasDuration
                  ? (details) => _updateDrag(details.localPosition, constraints.maxWidth)
                  : null,
              onTapUp: hasDuration
                  ? (details) {
                      _updateDrag(details.localPosition, constraints.maxWidth);
                      _seekTo(_dragFraction ?? 0);
                      setState(() => _dragFraction = null);
                    }
                  : null,
              onHorizontalDragStart: hasDuration
                  ? (details) => _updateDrag(details.localPosition, constraints.maxWidth)
                  : null,
              onHorizontalDragUpdate: hasDuration
                  ? (details) => _updateDrag(details.localPosition, constraints.maxWidth)
                  : null,
              onHorizontalDragEnd: hasDuration
                  ? (_) {
                      _seekTo(_dragFraction ?? 0);
                      setState(() => _dragFraction = null);
                    }
                  : null,
              child: SizedBox(
                height: height,
                child: Stack(
                  clipBehavior: Clip.none,
                  alignment: Alignment.center,
                  children: [
                    AnimatedBuilder(
                      animation: _ticker,
                      builder: (context, _) => CustomPaint(
                        size: Size(constraints.maxWidth, height),
                        painter: _SeekPainter(
                          fraction: _displayed,
                          active: hasDuration,
                          hovered: _hovered || _dragFraction != null,
                          trackColor: colors.border,
                          activeColor: colors.primary,
                          thumbColor: colors.primary,
                          glowColor: colors.primary.withValues(alpha: .25),
                          trackHeight: widget.compact ? 3 : 4,
                        ),
                      ),
                    ),
                    // 拖动时在滑块旁显示目标时间
                    if (_dragFraction != null)
                      Positioned(
                        left: _displayed * constraints.maxWidth - 34,
                        top: -18,
                        child: IgnorePointer(
                          child: DecoratedBox(
                            decoration: BoxDecoration(
                              color: colors.primary,
                              borderRadius: const BorderRadius.all(
                                Radius.circular(6),
                              ),
                            ),
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 8,
                                vertical: 2,
                              ),
                              child: Text(
                                formatDuration(
                                  Duration(
                                    milliseconds:
                                        (duration.inMilliseconds *
                                                _displayed)
                                            .round(),
                                  ),
                                ),
                                style: type.body.xs.copyWith(
                                  color: colors.primaryForeground,
                                  fontWeight: FontWeight.w600,
                                  fontFeatures: const [
                                    FontFeature.tabularFigures(),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
        if (widget.showTimes)
          AnimatedBuilder(
            animation: _ticker,
            builder: (context, _) {
              final displayed = Duration(
                milliseconds: (duration.inMilliseconds * _displayed).round(),
              );
              return Padding(
                padding: const EdgeInsets.symmetric(horizontal: 2),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      formatDuration(displayed),
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                        fontFeatures: const [FontFeature.tabularFigures()],
                      ),
                    ),
                    Text(
                      formatDuration(duration),
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                        fontFeatures: const [FontFeature.tabularFigures()],
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
      ],
    );
  }
}

class _SeekPainter extends CustomPainter {
  _SeekPainter({
    required this.fraction,
    required this.active,
    required this.hovered,
    required this.trackColor,
    required this.activeColor,
    required this.thumbColor,
    required this.glowColor,
    required this.trackHeight,
  });

  final double fraction;
  final bool active;
  final bool hovered;
  final Color trackColor;
  final Color activeColor;
  final Color thumbColor;
  final Color glowColor;
  final double trackHeight;

  @override
  void paint(Canvas canvas, Size size) {
    final centerY = size.height / 2;
    final radius = Radius.circular(trackHeight / 2);
    final track = RRect.fromRectAndRadius(
      Rect.fromLTWH(0, centerY - trackHeight / 2, size.width, trackHeight),
      radius,
    );
    canvas.drawRRect(track, Paint()..color = trackColor);
    if (!active) return;
    final filled = (size.width * fraction).clamp(0.0, size.width);
    if (filled > 0) {
      final activeTrack = RRect.fromRectAndRadius(
        Rect.fromLTWH(0, centerY - trackHeight / 2, filled, trackHeight),
        radius,
      );
      canvas.drawRRect(activeTrack, Paint()..color = activeColor);
    }
    final thumbRadius = hovered ? 6.5 : 5.0;
    final thumbCenter = Offset(filled, centerY);
    if (hovered) {
      canvas.drawCircle(
        thumbCenter,
        thumbRadius + 5,
        Paint()..color = glowColor,
      );
    }
    canvas.drawCircle(thumbCenter, thumbRadius, Paint()..color = thumbColor);
  }

  @override
  bool shouldRepaint(covariant _SeekPainter old) =>
      old.fraction != fraction ||
      old.active != active ||
      old.hovered != hovered ||
      old.trackColor != trackColor ||
      old.activeColor != activeColor;
}

/// 圆形图标按钮，桌面端悬停时轻微放大。
class RoundIconButton extends StatefulWidget {
  const RoundIconButton({
    super.key,
    required this.icon,
    required this.onPress,
    this.tooltip,
    this.size = 36,
    this.iconSize = 18,
    this.selected = false,
    this.filled = false,
  });

  final IconData icon;
  final VoidCallback? onPress;
  final String? tooltip;
  final double size;
  final double iconSize;
  final bool selected;
  final bool filled;

  @override
  State<RoundIconButton> createState() => _RoundIconButtonState();
}

class _RoundIconButtonState extends State<RoundIconButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final enabled = widget.onPress != null;
    final background = widget.filled
        ? colors.primary
        : widget.selected
        ? colors.primary.withValues(alpha: .16)
        : _hovered
        ? colors.secondary
        : const Color(0x00000000);
    final foreground = widget.filled
        ? colors.primaryForeground
        : widget.selected
        ? colors.primary
        : enabled
        ? colors.foreground
        : colors.mutedForeground.withValues(alpha: .5);
    final button = MouseRegion(
      cursor: enabled ? SystemMouseCursors.click : MouseCursor.defer,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onPress,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOutCubic,
          width: widget.size,
          height: widget.size,
          decoration: BoxDecoration(
            color: background,
            shape: BoxShape.circle,
            boxShadow: widget.filled
                ? [
                    BoxShadow(
                      color: colors.primary.withValues(alpha: .28),
                      blurRadius: _hovered ? 22 : 14,
                      spreadRadius: _hovered ? 3 : 1,
                    ),
                  ]
                : null,
          ),
          child: AnimatedScale(
            scale: _hovered && enabled ? 1.08 : 1,
            duration: const Duration(milliseconds: 160),
            curve: Curves.easeOutCubic,
            child: Icon(widget.icon, size: widget.iconSize, color: foreground),
          ),
        ),
      ),
    );
    final tooltip = widget.tooltip;
    if (tooltip == null) return button;
    return FTooltip(
      tipBuilder: (context, _) => Text(tooltip),
      child: button,
    );
  }
}

/// 上一首 / 播放 / 下一首。
class TransportControls extends StatelessWidget {
  const TransportControls({super.key, this.compact = false});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final size = compact ? 34.0 : 42.0;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        RoundIconButton(
          icon: _modeIcon(model.playbackMode),
          tooltip: '${model.playbackMode.label}（点击切换）',
          selected: model.playbackMode != PlaybackMode.sequence,
          size: size - 4,
          iconSize: compact ? 15 : 17,
          onPress: model.cyclePlaybackMode,
        ),
        SizedBox(width: compact ? 2 : 6),
        RoundIconButton(
          icon: FLucideIcons.skipBack,
          tooltip: '上一首',
          size: size,
          iconSize: compact ? 18 : 20,
          onPress: model.current == null ? null : model.playPrevious,
        ),
        SizedBox(width: compact ? 4 : 8),
        _PlayButton(playing: model.isPlaying, size: size + 8, onPress: model.togglePlayback),
        SizedBox(width: compact ? 4 : 8),
        RoundIconButton(
          icon: FLucideIcons.skipForward,
          tooltip: '下一首',
          size: size,
          iconSize: compact ? 18 : 20,
          onPress: model.current == null ? null : model.playNext,
        ),
      ],
    );
  }

  IconData _modeIcon(PlaybackMode mode) => switch (mode) {
    PlaybackMode.sequence => FLucideIcons.listMusic,
    PlaybackMode.listLoop => FLucideIcons.repeat,
    PlaybackMode.shuffle => FLucideIcons.shuffle,
    PlaybackMode.singleLoop => FLucideIcons.repeat1,
  };
}

class _PlayButton extends StatefulWidget {
  const _PlayButton({
    required this.playing,
    required this.size,
    required this.onPress,
  });

  final bool playing;
  final double size;
  final VoidCallback onPress;

  @override
  State<_PlayButton> createState() => _PlayButtonState();
}

class _PlayButtonState extends State<_PlayButton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _breath = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 4),
  );

  bool _hovered = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncBreath();
  }

  @override
  void didUpdateWidget(_PlayButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncBreath();
  }

  void _syncBreath() {
    final enabled =
        widget.playing &&
        !MediaQuery.disableAnimationsOf(context) &&
        TickerMode.valuesOf(context).enabled;
    if (enabled) {
      if (!_breath.isAnimating) _breath.repeat(reverse: true);
    } else {
      _breath
        ..stop()
        ..value = 0;
    }
  }

  @override
  void dispose() {
    _breath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onPress,
        child: AnimatedBuilder(
          animation: _breath,
          builder: (context, child) {
            final glow = .18 + _breath.value * .22;
            return Transform.scale(
              scale: _hovered ? 1.06 : 1 + _breath.value * .012,
              child: Container(
                width: widget.size,
                height: widget.size,
                decoration: BoxDecoration(
                  color: colors.primary,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: colors.primary.withValues(alpha: glow),
                      blurRadius: 24 + _breath.value * 10,
                      spreadRadius: 2,
                    ),
                  ],
                ),
                child: child,
              ),
            );
          },
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 180),
            switchInCurve: Curves.easeOutCubic,
            switchOutCurve: Curves.easeInCubic,
            transitionBuilder: (child, animation) => ScaleTransition(
              scale: animation,
              child: FadeTransition(opacity: animation, child: child),
            ),
            child: Icon(
              widget.playing ? FLucideIcons.pause : FLucideIcons.play,
              key: ValueKey(widget.playing),
              size: widget.size * .46,
              color: colors.primaryForeground,
            ),
          ),
        ),
      ),
    );
  }
}

/// 音量：静音 + 滑块 + 百分比。
class VolumeControl extends StatelessWidget {
  const VolumeControl({super.key, this.width = 170, this.showLabel = true});

  final double width;
  final bool showLabel;

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return SizedBox(
      width: width,
      child: Row(
        children: [
          RoundIconButton(
            icon: _icon(model.volume),
            tooltip: model.volume <= .001 ? '取消静音' : '静音',
            size: 30,
            iconSize: 16,
            onPress: model.toggleMute,
          ),
          const SizedBox(width: 4),
          Expanded(
            child: FSlider(
              control: FSliderControl.liftedContinuous(
                value: FSliderValue(max: model.volume.clamp(0.0, 1.0)),
                onChange: (value) => model.setVolume(value.max),
              ),
              tooltipBuilder: (controller, value) =>
                  Text('${(value * 100).round()}%'),
            ),
          ),
          if (showLabel)
            SizedBox(
              width: 46,
              child: Text(
                '${(model.volume * 100).round()}%',
                maxLines: 1,
                textAlign: TextAlign.end,
                style: type.body.xs.copyWith(
                  color: colors.mutedForeground,
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
            ),
        ],
      ),
    );
  }

  IconData _icon(double volume) {
    if (volume <= .001) return FLucideIcons.volumeX;
    if (volume < .5) return FLucideIcons.volume1;
    return FLucideIcons.volume2;
  }
}

