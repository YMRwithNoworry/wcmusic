import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

/// 统一的悬停反馈：轻微抬升 + 边框/底色过渡。
///
/// 只重建自身，不触发父级布局；用于封面卡片、列表行等高频悬停区域。
class HoverLift extends StatefulWidget {
  const HoverLift({
    super.key,
    required this.child,
    this.onTap,
    this.onSecondaryTap,
    this.onLongPress,
    this.scale = 1.015,
    this.translateY = -2,
    this.borderRadius = const BorderRadius.all(Radius.circular(14)),
    this.cursor = SystemMouseCursors.click,
    this.enabled = true,
  });

  final Widget child;
  final VoidCallback? onTap;
  final VoidCallback? onSecondaryTap;
  final VoidCallback? onLongPress;
  final double scale;
  final double translateY;
  final BorderRadius borderRadius;
  final MouseCursor cursor;
  final bool enabled;

  @override
  State<HoverLift> createState() => _HoverLiftState();
}

class _HoverLiftState extends State<HoverLift> {
  bool _hovered = false;

  void _setHovered(bool value) {
    if (_hovered == value || !widget.enabled) return;
    setState(() => _hovered = value);
  }

  @override
  Widget build(BuildContext context) {
    final interactive =
        widget.enabled &&
        (widget.onTap != null ||
            widget.onSecondaryTap != null ||
            widget.onLongPress != null);
    return MouseRegion(
      cursor: interactive ? widget.cursor : MouseCursor.defer,
      onEnter: (_) => _setHovered(true),
      onExit: (_) => _setHovered(false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.enabled ? widget.onTap : null,
        onLongPress: widget.enabled ? widget.onLongPress : null,
        onSecondaryTap: widget.enabled ? widget.onSecondaryTap : null,
        child: AnimatedScale(
          scale: _hovered ? widget.scale : 1,
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          child: widget.child,
        ),
      ),
    );
  }
}

/// 只在桌面端响应右键；触屏长按走 onLongPress。
bool isSecondaryClick(PointerDownEvent event) =>
    event.buttons == kSecondaryMouseButton;
