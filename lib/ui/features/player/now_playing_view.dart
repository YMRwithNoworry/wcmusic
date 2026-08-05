import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/organic_artwork.dart';
import 'playback_controls.dart';
import 'player_view_model.dart';

class NowPlayingView extends StatelessWidget {
  const NowPlayingView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final track = viewModel.current;
    if (track == null) return const SizedBox.shrink();
    return Scaffold(
      body: SafeArea(
        child: Stack(
          children: [
            Positioned.fill(child: _AmbientField(seed: track.id)),
            Positioned(
              left: 12,
              top: 10,
              child: IconButton.filledTonal(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.keyboard_arrow_down),
                tooltip: '收起',
              ),
            ),
            Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.symmetric(
                  horizontal: 24,
                  vertical: 56,
                ),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 610),
                  child: Column(
                    children: [
                      LayoutBuilder(
                        builder: (context, constraints) {
                          final size = constraints.maxWidth.clamp(260.0, 520.0);
                          return Hero(
                            tag: 'art-${track.id}',
                            child: OrganicArtwork(
                              seed: track.id,
                              size: size,
                              playing: viewModel.isPlaying,
                            ),
                          );
                        },
                      ),
                      const SizedBox(height: 32),
                      Text(
                        track.title,
                        textAlign: TextAlign.center,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.displayMedium,
                      ),
                      const SizedBox(height: 8),
                      Text(
                        track.artist,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                      const SizedBox(height: 30),
                      const PlaybackProgress(),
                      const SizedBox(height: 18),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.shuffle),
                            tooltip: '随机播放',
                          ),
                          const SizedBox(width: 14),
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.skip_previous, size: 34),
                            tooltip: '上一首',
                          ),
                          const SizedBox(width: 14),
                          IconButton.filled(
                            iconSize: 34,
                            onPressed: viewModel.togglePlayback,
                            icon: Icon(
                              viewModel.isPlaying
                                  ? Icons.pause
                                  : Icons.play_arrow,
                            ),
                            tooltip: viewModel.isPlaying ? '暂停' : '播放',
                          ),
                          const SizedBox(width: 14),
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.skip_next, size: 34),
                            tooltip: '下一首',
                          ),
                          const SizedBox(width: 14),
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.repeat),
                            tooltip: '循环模式',
                          ),
                        ],
                      ),
                      const SizedBox(height: 18),
                      const SizedBox(width: 300, child: VolumeControl()),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _AmbientField extends StatelessWidget {
  const _AmbientField({required this.seed});

  final String seed;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: dark ? const Color(0xFF1A211B) : const Color(0xFFE6E4D8),
      ),
      child: CustomPaint(painter: _FiberPainter(seed.hashCode, dark)),
    );
  }
}

class _FiberPainter extends CustomPainter {
  const _FiberPainter(this.seed, this.dark);

  final int seed;
  final bool dark;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = (dark ? Colors.white : Colors.black).withValues(alpha: .025)
      ..strokeWidth = .7;
    for (var index = 0; index < 84; index++) {
      final y = size.height * index / 84;
      final drift = ((seed + index * 37) % 29).toDouble();
      canvas.drawLine(
        Offset(-drift, y),
        Offset(size.width, y + drift * .2),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _FiberPainter oldDelegate) =>
      oldDelegate.seed != seed || oldDelegate.dark != dark;
}
