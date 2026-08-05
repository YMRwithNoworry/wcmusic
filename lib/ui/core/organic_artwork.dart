import 'dart:math' as math;

import 'package:flutter/material.dart';

class OrganicArtwork extends StatefulWidget {
  const OrganicArtwork({
    super.key,
    required this.seed,
    this.size = 120,
    this.playing = false,
  });

  final String seed;
  final double size;
  final bool playing;

  @override
  State<OrganicArtwork> createState() => _OrganicArtworkState();
}

class _OrganicArtworkState extends State<OrganicArtwork>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 8),
  );

  @override
  void initState() {
    super.initState();
    if (widget.playing) _controller.repeat(reverse: true);
  }

  @override
  void didUpdateWidget(covariant OrganicArtwork oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.playing == widget.playing) return;
    widget.playing
        ? _controller.repeat(reverse: true)
        : _controller.animateBack(0);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final artwork = RepaintBoundary(
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: AnimatedBuilder(
          animation: _controller,
          builder: (_, _) => CustomPaint(
            painter: _ArtworkPainter(widget.seed.hashCode, _controller.value),
          ),
        ),
      ),
    );
    if (widget.size.isInfinite) return SizedBox.expand(child: artwork);
    return SizedBox.square(dimension: widget.size, child: artwork);
  }
}

class _ArtworkPainter extends CustomPainter {
  _ArtworkPainter(this.seed, this.phase);

  final int seed;
  final double phase;

  @override
  void paint(Canvas canvas, Size size) {
    final random = math.Random(seed);
    final palettes = <List<Color>>[
      [
        const Color(0xFF49624B),
        const Color(0xFFD9B26F),
        const Color(0xFFF2E9D8),
      ],
      [
        const Color(0xFF315E64),
        const Color(0xFFD56F52),
        const Color(0xFFF2C98B),
      ],
      [
        const Color(0xFF6B5948),
        const Color(0xFF8FAE8B),
        const Color(0xFFE7D8C2),
      ],
      [
        const Color(0xFF384957),
        const Color(0xFFB55D62),
        const Color(0xFFD8C9A7),
      ],
    ];
    final colors = palettes[seed.abs() % palettes.length];
    canvas.drawRect(Offset.zero & size, Paint()..color = colors.first);
    for (var index = 0; index < 6; index++) {
      final radius =
          size.width * (.16 + random.nextDouble() * .28) * (1 + phase * .025);
      final center = Offset(
        size.width * (random.nextDouble() * .9 + .05),
        size.height * (random.nextDouble() * .9 + .05),
      );
      canvas.drawCircle(
        center,
        radius,
        Paint()
          ..color = colors[1 + index % 2].withValues(alpha: .42 + index * .055),
      );
    }
    final grain = Paint()..color = Colors.white.withValues(alpha: .12);
    for (var index = 0; index < 180; index++) {
      canvas.drawCircle(
        Offset(
          random.nextDouble() * size.width,
          random.nextDouble() * size.height,
        ),
        random.nextDouble() * .7 + .2,
        grain,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _ArtworkPainter oldDelegate) =>
      oldDelegate.phase != phase || oldDelegate.seed != seed;
}
