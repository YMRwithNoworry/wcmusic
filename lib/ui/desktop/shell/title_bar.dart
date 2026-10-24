import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:window_manager/window_manager.dart';


/// 自绘标题栏：左侧品牌、右侧最小化 / 最大化 / 关闭。
///
/// 窗口使用 [TitleBarStyle.hidden]，所以拖动、双击最大化都要自己处理。
class DesktopTitleBar extends StatefulWidget {
  const DesktopTitleBar({super.key, this.subtitle});

  final String? subtitle;

  @override
  State<DesktopTitleBar> createState() => _DesktopTitleBarState();
}

class _DesktopTitleBarState extends State<DesktopTitleBar> with WindowListener {
  bool _maximized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    windowManager
        .isMaximized()
        .then((value) {
          if (mounted) setState(() => _maximized = value);
        })
        .catchError((Object _) {});
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowMaximize() => setState(() => _maximized = true);

  @override
  void onWindowUnmaximize() => setState(() => _maximized = false);

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.card,
        border: Border(bottom: BorderSide(color: colors.border)),
      ),
      child: SizedBox(
        height: 40,
        child: Row(
          children: [
            const SizedBox(width: 16),
            Image.asset(
              'assets/icons/app_icon.png',
              width: 20,
              height: 20,
              filterQuality: FilterQuality.medium,
            ),
            const SizedBox(width: 9),
            Text(
              'WCMusic',
              style: type.body.xs.copyWith(
                color: colors.foreground,
                fontWeight: FontWeight.w600,
                letterSpacing: .8,
              ),
            ),
            if (widget.subtitle != null) ...[
              const SizedBox(width: 10),
              Text(
                '/ ${widget.subtitle}',
                style: type.body.xs.copyWith(
                  color: colors.mutedForeground,
                  letterSpacing: .4,
                ),
              ),
            ],
            Expanded(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onPanStart: (_) => windowManager.startDragging(),
                onDoubleTap: _toggleMaximize,
              ),
            ),
            _CaptionButton(
              icon: FLucideIcons.minus,
              tooltip: '最小化',
              onPress: windowManager.minimize,
            ),
            _CaptionButton(
              icon: _maximized ? FLucideIcons.copy : FLucideIcons.square,
              tooltip: _maximized ? '还原' : '最大化',
              onPress: _toggleMaximize,
            ),
            _CaptionButton(
              icon: FLucideIcons.x,
              tooltip: '关闭到托盘',
              danger: true,
              onPress: windowManager.close,
            ),
            const SizedBox(width: 4),
          ],
        ),
      ),
    );
  }

  Future<void> _toggleMaximize() async {
    if (await windowManager.isMaximized()) {
      await windowManager.unmaximize();
    } else {
      await windowManager.maximize();
    }
  }
}

class _CaptionButton extends StatefulWidget {
  const _CaptionButton({
    required this.icon,
    required this.tooltip,
    required this.onPress,
    this.danger = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback onPress;
  final bool danger;

  @override
  State<_CaptionButton> createState() => _CaptionButtonState();
}

class _CaptionButtonState extends State<_CaptionButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final hoverColor = widget.danger
        ? colors.destructive
        : colors.secondary.withValues(alpha: .8);
    return FTooltip(
      tipBuilder: (context, _) => Text(widget.tooltip),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          onTap: widget.onPress,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            width: 46,
            height: 40,
            color: _hovered ? hoverColor : const Color(0x00000000),
            child: Icon(
              widget.icon,
              size: 15,
              color: _hovered && widget.danger ? colors.destructiveForeground : colors.mutedForeground,
            ),
          ),
        ),
      ),
    );
  }
}
