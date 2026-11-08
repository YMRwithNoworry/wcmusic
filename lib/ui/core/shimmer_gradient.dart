import 'package:flutter/material.dart';

class ShimmerGradient extends StatefulWidget {
  const ShimmerGradient({
    super.key,
    required this.child,
    this.enabled = true,
    this.colors,
  });

  final Widget child;
  final bool enabled;
  final List<Color>? colors;

  @override
  State<ShimmerGradient> createState() => _ShimmerGradientState();
}

class _ShimmerGradientState extends State<ShimmerGradient>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 2800),
    );
    if (widget.enabled) _controller.repeat();
  }

  @override
  void didUpdateWidget(ShimmerGradient oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.enabled != oldWidget.enabled) {
      if (widget.enabled) {
        _controller.repeat();
      } else {
        _controller.stop();
      }
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.enabled) return widget.child;
    
    final colors = widget.colors ??
        [
          Theme.of(context).colorScheme.surface.withValues(alpha: 0.3),
          Theme.of(context).colorScheme.primary.withValues(alpha: 0.15),
          Theme.of(context).colorScheme.tertiary.withValues(alpha: 0.12),
          Theme.of(context).colorScheme.surface.withValues(alpha: 0.3),
        ];

    return AnimatedBuilder(
      animation: _controller,
      builder: (context, child) {
        final progress = _controller.value;
        return Container(
          foregroundDecoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment(-1.5 + progress * 3, -0.5),
              end: Alignment(-0.5 + progress * 3, 0.5),
              colors: colors,
              stops: const [0.0, 0.3, 0.6, 1.0],
            ),
          ),
          child: child,
        );
      },
      child: widget.child,
    );
  }
}
