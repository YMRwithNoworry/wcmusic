import 'dart:math' as math;
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/track.dart';
import '../../core/breathing_widget.dart';
import '../../core/organic_artwork.dart';
import 'playback_mode_button.dart';
import 'playback_controls.dart';
import 'player_view_model.dart';

class NowPlayingView extends StatelessWidget {
  const NowPlayingView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final track = viewModel.current;
    if (track == null) return const SizedBox.shrink();
    final baseTheme = Theme.of(context);
    final focusedTheme = baseTheme.copyWith(
      brightness: Brightness.dark,
      colorScheme: ColorScheme.fromSeed(
        seedColor: baseTheme.colorScheme.primary,
        brightness: Brightness.dark,
      ),
      textTheme: baseTheme.textTheme.apply(
        bodyColor: Colors.white,
        displayColor: Colors.white,
      ),
    );

    return Theme(
      data: focusedTheme,
      child: Scaffold(
        backgroundColor: Colors.black,
        body: Stack(
          children: [
            Positioned.fill(
              child: AnimatedSwitcher(
                duration: const Duration(milliseconds: 900),
                child: _DynamicArtworkBackdrop(
                  key: ValueKey(track.id),
                  track: track,
                ),
              ),
            ),
            Positioned.fill(
              child: ColoredBox(color: Colors.black.withValues(alpha: .34)),
            ),
            SafeArea(
              child: Stack(
                children: [
                  Positioned(
                    left: 12,
                    top: 10,
                    child: IconButton.filledTonal(
                      onPressed: () => Navigator.pop(context),
                      icon: const Icon(Icons.close),
                      tooltip: '关闭聚焦播放',
                    ),
                  ),
                  Positioned.fill(child: _FocusedPlayer(track: track)),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _FocusedPlayer extends StatelessWidget {
  const _FocusedPlayer({required this.track});

  final Track track;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return LayoutBuilder(
      builder: (context, constraints) {
        final widthBound = (constraints.maxWidth - 48).clamp(260.0, 560.0);
        final heightBound = (constraints.maxHeight - 310).clamp(240.0, 560.0);
        final artworkSize = math.min(widthBound, heightBound);
        return Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(24, 64, 24, 32),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 640),
              child: Column(
                children: [
                  BreathingWidget(
                    enabled: viewModel.isPlaying,
                    duration: const Duration(seconds: 5),
                    scaleAmplitude: 0.015,
                    opacityAmplitude: 0.03,
                    child: Hero(
                      tag: 'art-${track.id}',
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                          boxShadow: [
                            BoxShadow(
                              color: Colors.black.withValues(alpha: .36),
                              blurRadius: 42,
                              offset: const Offset(0, 20),
                            ),
                            BoxShadow(
                              color: Theme.of(context)
                                  .colorScheme
                                  .primary
                                  .withValues(alpha: .12),
                              blurRadius: 64,
                              spreadRadius: 8,
                            ),
                          ],
                        ),
                        child: OrganicArtwork(
                          seed: track.id,
                          size: artworkSize,
                          playing: viewModel.isPlaying,
                          artworkUri: track.artworkUri,
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 28),
                  Text(
                    track.title,
                    textAlign: TextAlign.center,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    track.artist,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      color: Colors.white.withValues(alpha: .76),
                    ),
                  ),
                  const SizedBox(height: 22),
                  const PlaybackProgress(),
                  const SizedBox(height: 12),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const PlaybackModeButton(),
                      const SizedBox(width: 12),
                      SpringButton(
                        onTap: viewModel.playPrevious,
                        child: IconButton(
                          onPressed: null,
                          icon: const Icon(Icons.skip_previous, size: 34),
                          tooltip: '上一首',
                        ),
                      ),
                      const SizedBox(width: 12),
                      BreathingWidget(
                        enabled: viewModel.isPlaying,
                        scaleAmplitude: 0.08,
                        child: SpringButton(
                          onTap: viewModel.togglePlayback,
                          child: IconButton.filled(
                            iconSize: 34,
                            onPressed: null,
                            icon: Icon(
                              viewModel.isPlaying
                                  ? Icons.pause
                                  : Icons.play_arrow,
                            ),
                            tooltip: viewModel.isPlaying ? '暂停' : '播放',
                          ),
                        ),
                      ),
                      const SizedBox(width: 12),
                      SpringButton(
                        onTap: viewModel.playNext,
                        child: IconButton(
                          onPressed: null,
                          icon: const Icon(Icons.skip_next, size: 34),
                          tooltip: '下一首',
                        ),
                      ),
                      const SizedBox(width: 12),
                      const FloatingLyricsButton(),
                      const SizedBox(width: 6),
                      const DownloadButton(),
                    ],
                  ),
                  const SizedBox(height: 12),
                  const SizedBox(width: 320, child: VolumeControl()),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

class _DynamicArtworkBackdrop extends StatefulWidget {
  const _DynamicArtworkBackdrop({super.key, required this.track});

  final Track track;

  @override
  State<_DynamicArtworkBackdrop> createState() =>
      _DynamicArtworkBackdropState();
}

class _DynamicArtworkBackdropState extends State<_DynamicArtworkBackdrop>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 24),
  );

  late final Animation<double> _flowAnimation = CurvedAnimation(
    parent: _controller,
    curve: Curves.easeInOutSine,
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      _controller.stop();
      _controller.value = .5;
    } else if (!_controller.isAnimating) {
      _controller.repeat(reverse: true);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ClipRect(
      child: AnimatedBuilder(
        animation: _flowAnimation,
        builder: (context, child) {
          final phase = _flowAnimation.value;
          return Transform.translate(
            offset: Offset(-24 + phase * 48, 16 - phase * 32),
            child: Transform.scale(
              scale: 1.35 + phase * .10,
              child: ImageFiltered(
                imageFilter: ImageFilter.blur(
                  sigmaX: 64 + phase * 8,
                  sigmaY: 64 + phase * 8,
                ),
                child: child,
              ),
            ),
          );
        },
        child: SizedBox.expand(
          child: OrganicArtwork(
            seed: widget.track.id,
            size: double.infinity,
            playing: true,
            artworkUri: widget.track.artworkUri,
          ),
        ),
      ),
    );
  }
}
