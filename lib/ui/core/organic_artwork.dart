import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'app_theme.dart';

/// 程序化封面。
///
/// 没有封面图时用种子生成一块安静的渐变，避免列表里出现空洞。绘制用的随机点
/// 会按种子缓存，动画时只改变相位，不重新生成几何数据。
class OrganicArtwork extends StatefulWidget {
  const OrganicArtwork({
    super.key,
    required this.seed,
    this.size = 120,
    this.playing = false,
    this.artworkUri,
  });

  final String seed;
  final double size;
  final bool playing;
  final String? artworkUri;

  @override
  State<OrganicArtwork> createState() => _OrganicArtworkState();
}

class _OrganicArtworkState extends State<OrganicArtwork>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 14),
  );

  late final Animation<double> _breathAnimation = CurvedAnimation(
    parent: _controller,
    curve: Curves.easeInOutSine,
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncAnimation();
  }

  @override
  void didUpdateWidget(covariant OrganicArtwork oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.playing == widget.playing) return;
    _syncAnimation();
  }

  void _syncAnimation() {
    if (MediaQuery.disableAnimationsOf(context) || !widget.playing) {
      _controller
        ..stop()
        ..value = 0;
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
    final fallback = RepaintBoundary(
      child: AnimatedBuilder(
        animation: _breathAnimation,
        builder: (_, _) => CustomPaint(
          painter: _ArtworkPainter(widget.seed.hashCode, _breathAnimation.value),
        ),
      ),
    );
    final uri = widget.artworkUri;
    final artwork = ClipRRect(
      borderRadius: BorderRadius.circular(10),
      child: uri == null || uri.isEmpty
          ? fallback
          : Image.network(
              uri,
              width: double.infinity,
              height: double.infinity,
              fit: BoxFit.cover,
              gaplessPlayback: true,
              filterQuality: FilterQuality.medium,
              headers: const {
                'User-Agent': 'Mozilla/5.0',
                'Referer': 'https://music.163.com/',
              },
              loadingBuilder: (context, child, progress) =>
                  progress == null ? child : fallback,
              errorBuilder: (_, _, _) => fallback,
            ),
    );
    if (widget.size.isInfinite) return SizedBox.expand(child: artwork);
    return SizedBox.square(dimension: widget.size, child: artwork);
  }
}

class _ArtworkSpec {
  _ArtworkSpec(int seed) {
    final random = math.Random(seed);
    final palettes = AppPalette.artworkPalettes;
    colors = palettes[seed.abs() % palettes.length];
    circles = [
      for (var index = 0; index < 7; index++)
        (
          x: random.nextDouble() * .9 + .05,
          y: random.nextDouble() * .9 + .05,
          radius: .14 + random.nextDouble() * .32,
          alpha: .38 + index * .048,
        ),
    ];
    grain = [
      for (var index = 0; index < 160; index++)
        (
          x: random.nextDouble(),
          y: random.nextDouble(),
          radius: random.nextDouble() * .8 + .3,
        ),
    ];
  }

  late final List<Color> colors;
  late final List<({double x, double y, double radius, double alpha})> circles;
  late final List<({double x, double y, double radius})> grain;

  static final Map<int, _ArtworkSpec> _cache = {};

  static _ArtworkSpec of(int seed) {
    final cached = _cache[seed];
    if (cached != null) return cached;
    if (_cache.length >= 64) _cache.remove(_cache.keys.first);
    return _cache[seed] = _ArtworkSpec(seed);
  }
}

class _ArtworkPainter extends CustomPainter {
  _ArtworkPainter(this.seed, this.phase);

  final int seed;
  final double phase;

  @override
  void paint(Canvas canvas, Size size) {
    final spec = _ArtworkSpec.of(seed);
    final colors = spec.colors;
    canvas.drawRect(Offset.zero & size, Paint()..color = colors.first);
    final circlePaint = Paint();
    for (final circle in spec.circles) {
      circlePaint.color = colors[1 + circle.radius.round() % 2].withValues(
        alpha: (circle.alpha + phase * .08).clamp(0.0, 1.0),
      );
      canvas.drawCircle(
        Offset(circle.x * size.width, circle.y * size.height),
        size.width * circle.radius * (1 + phase * .035),
        circlePaint,
      );
    }
    final grainPaint = Paint()
      ..color = Colors.white.withValues(alpha: .10 + phase * .04);
    for (final grain in spec.grain) {
      canvas.drawCircle(
        Offset(grain.x * size.width, grain.y * size.height),
        grain.radius,
        grainPaint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _ArtworkPainter oldDelegate) =>
      oldDelegate.phase != phase || oldDelegate.seed != seed;
}
