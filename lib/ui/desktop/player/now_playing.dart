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

/// 聚焦播放：歌词与完整控制。
///
/// 作为内容区的一层覆盖而不是新路由，切页时不重建侧栏与播放条；进入/退出只做
/// 透明度 + 位移 + 轻微缩放，避免整页布局重排。底部播放条已经提供进度与音量，
/// 这里只负责把歌词放大展示。封面色块只作为背景模糊。
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
                    final wide = constraints.maxWidth >= 780;
                    return wide
                        ? const _WideLayout()
                        : const _CompactLayout();
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
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 6),
      child: Align(
        alignment: Alignment.centerLeft,
        child: FButton(
          variant: FButtonVariant.ghost,
          size: FButtonSizeVariant.sm,
          mainAxisSize: MainAxisSize.min,
          prefix: const Icon(FLucideIcons.chevronLeft, size: 16),
          onPress: onClose,
          child: const Text('返回'),
        ),
      ),
    );
  }
}

class _WideLayout extends StatelessWidget {
  const _WideLayout();

  @override
  Widget build(BuildContext context) {
    return Padding(
      // 详情页不重复展示封面、标题、进度与音量（都在底部播放条上），只放大
      // 歌词并铺满内容区。
      padding: const EdgeInsets.fromLTRB(48, 18, 48, 34),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 760),
          child: Align(
            alignment: Alignment.centerLeft,
            child: SizedBox.expand(child: LyricsPane(maxLines: 2)),
          ),
        ),
      ),
    );
  }
}

class _CompactLayout extends StatelessWidget {
  const _CompactLayout();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 4, 24, 20),
      child: Column(
        children: [
          const SizedBox(height: 8),
          const Expanded(child: LyricsPane(compact: true, maxLines: 1)),
          SeekBar(compact: true),
          const SizedBox(height: 6),
          const TransportControls(compact: true),
        ],
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
