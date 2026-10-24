import 'dart:ui' show ImageFilter;

import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/track.dart';
import '../../core/app_theme_controller.dart';
import '../../state/player_view_model.dart';
import '../widgets/artwork.dart';
import 'controls.dart';
import 'lyrics_view.dart';

/// 聚焦播放：大封面、歌词、完整控制。
///
/// 作为内容区的一层覆盖而不是新路由，切页时不重建侧栏与播放条；进入/退出只做
/// 透明度 + 位移 + 轻微缩放，避免整页布局重排。
class NowPlayingPanel extends StatelessWidget {
  const NowPlayingPanel({super.key, required this.onClose});

  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final track = model.current;
    if (track == null) {
      return const SizedBox.shrink();
    }
    final colors = context.theme.colors;
    final reduceMotion = context.watch<AppThemeController>().reduceMotion;
    return DecoratedBox(
      decoration: BoxDecoration(color: colors.background),
      child: Stack(
        fit: StackFit.expand,
        children: [
          _AnimatedBackdrop(track: track, reduceMotion: reduceMotion),
          DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  colors.background.withValues(alpha: .55),
                  colors.background.withValues(alpha: .78),
                  colors.background.withValues(alpha: .96),
                ],
              ),
            ),
          ),
          Column(
            children: [
              _TopBar(onClose: onClose),
              Expanded(
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final wide = constraints.maxWidth >= 980;
                    return wide
                        ? _WideLayout(track: track)
                        : _CompactLayout(track: track);
                  },
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _TopBar extends StatelessWidget {
  const _TopBar({required this.onClose});

  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final track = context.select<PlayerViewModel, Track?>(
      (model) => model.current,
    );
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 6),
      child: Row(
        children: [
          FButton(
            variant: FButtonVariant.ghost,
            size: FButtonSizeVariant.sm,
            mainAxisSize: MainAxisSize.min,
            prefix: const Icon(FLucideIcons.chevronLeft, size: 16),
            onPress: onClose,
            child: const Text('返回'),
          ),
          const SizedBox(width: 12),
          Text(
            '聚焦播放',
            style: type.body.xs.copyWith(
              color: colors.mutedForeground,
              letterSpacing: 1.6,
            ),
          ),
          const Spacer(),
          if (track?.quality != null) ...[
            FBadge(
              variant: FBadgeVariant.secondary,
              child: Text(track!.quality!),
            ),
            const SizedBox(width: 8),
          ],
          FButton.icon(
            variant: FButtonVariant.ghost,
            size: FButtonSizeVariant.sm,
            onPress: onClose,
            child: const Icon(FLucideIcons.x, size: 17),
          ),
        ],
      ),
    );
  }
}

class _WideLayout extends StatelessWidget {
  const _WideLayout({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Padding(
      padding: const EdgeInsets.fromLTRB(48, 18, 48, 34),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Hero(
                tag: 'art-${track.id}',
                child: _ArtworkHero(track: track),
              ),
              const SizedBox(height: 26),
              SizedBox(
                width: 340,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      track.title,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: type.display.xl3.copyWith(
                        color: colors.foreground,
                        fontWeight: FontWeight.w700,
                        letterSpacing: -.7,
                        height: 1.12,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      track.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.md.copyWith(
                        color: colors.foreground.withValues(alpha: .72),
                      ),
                    ),
                    if (track.album.isNotEmpty) ...[
                      const SizedBox(height: 3),
                      Text(
                        track.album,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: type.body.xs.copyWith(
                          color: colors.mutedForeground,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(width: 56),
          Expanded(
            child: Column(
              children: [
                const Expanded(child: LyricsPane()),
                const SizedBox(height: 8),
                SeekBar(compact: false),
                const SizedBox(height: 10),
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    TransportControls(),
                    const SizedBox(width: 22),
                    const VolumeControl(width: 190),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _CompactLayout extends StatelessWidget {
  const _CompactLayout({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 4, 24, 20),
      child: Column(
        children: [
          const SizedBox(height: 8),
          Hero(tag: 'art-${track.id}', child: _ArtworkHero(track: track, size: 200)),
          const SizedBox(height: 18),
          Text(
            track.title,
            maxLines: 2,
            textAlign: TextAlign.center,
            overflow: TextOverflow.ellipsis,
            style: type.display.xs.copyWith(
              color: colors.foreground,
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            track.artist,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: type.body.xs.copyWith(color: colors.mutedForeground),
          ),
          const SizedBox(height: 10),
          const Expanded(child: LyricsPane(compact: true)),
          SeekBar(compact: true),
          const SizedBox(height: 6),
          const TransportControls(compact: true),
        ],
      ),
    );
  }
}

class _ArtworkHero extends StatefulWidget {
  const _ArtworkHero({required this.track, this.size = 340});

  final Track track;
  final double size;

  @override
  State<_ArtworkHero> createState() => _ArtworkHeroState();
}

class _ArtworkHeroState extends State<_ArtworkHero>
    with SingleTickerProviderStateMixin {
  late final AnimationController _float = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 7),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final enabled =
        !MediaQuery.disableAnimationsOf(context) && TickerMode.valuesOf(context).enabled;
    if (enabled) {
      if (!_float.isAnimating) _float.repeat(reverse: true);
    } else {
      _float
        ..stop()
        ..value = .5;
    }
  }

  @override
  void dispose() {
    _float.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    return AnimatedBuilder(
      animation: _float,
      builder: (context, child) {
        final offset = (_float.value - .5) * 8;
        return Transform.translate(
          offset: Offset(0, offset),
          child: DecoratedBox(
            decoration: BoxDecoration(
              borderRadius: const BorderRadius.all(Radius.circular(18)),
              boxShadow: [
                BoxShadow(
                  color: const Color(0xFF000000).withValues(alpha: .42),
                  blurRadius: 48,
                  offset: const Offset(0, 24),
                ),
                BoxShadow(
                  color: colors.primary.withValues(alpha: .16),
                  blurRadius: 70,
                  spreadRadius: 6,
                ),
              ],
            ),
            child: child,
          ),
        );
      },
      child: ArtworkView(
        seed: widget.track.id,
        url: widget.track.artworkUri,
        size: widget.size,
        radius: 18,
        playing: true,
      ),
    );
  }
}

class _AnimatedBackdrop extends StatefulWidget {
  const _AnimatedBackdrop({required this.track, required this.reduceMotion});

  final Track track;
  final bool reduceMotion;

  @override
  State<_AnimatedBackdrop> createState() => _AnimatedBackdropState();
}

class _AnimatedBackdropState extends State<_AnimatedBackdrop>
    with SingleTickerProviderStateMixin {
  late final AnimationController _drift = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 26),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sync();
  }

  @override
  void didUpdateWidget(_AnimatedBackdrop oldWidget) {
    super.didUpdateWidget(oldWidget);
    _sync();
  }

  void _sync() {
    final enabled =
        !widget.reduceMotion &&
        !MediaQuery.disableAnimationsOf(context) &&
        TickerMode.valuesOf(context).enabled;
    if (enabled) {
      if (!_drift.isAnimating) _drift.repeat(reverse: true);
    } else {
      _drift
        ..stop()
        ..value = .5;
    }
  }

  @override
  void dispose() {
    _drift.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ClipRect(
      child: AnimatedBuilder(
        animation: _drift,
        builder: (context, child) {
          final phase = _drift.value;
          return Transform.translate(
            offset: Offset(-18 + phase * 36, 10 - phase * 20),
            child: Transform.scale(scale: 1.24 + phase * .08, child: child),
          );
        },
        child: RepaintBoundary(
          child: ImageFiltered(
            imageFilter: ImageFilter.blur(sigmaX: 46, sigmaY: 46),
            child: ArtworkView(
              seed: widget.track.id,
              url: widget.track.artworkUri,
              size: double.infinity,
              radius: 0,
            ),
          ),
        ),
      ),
    );
  }
}
