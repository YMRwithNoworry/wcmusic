import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';

import '../widgets/hover.dart';

/// 页面标题区：大标题 + 说明 + 右侧操作。
class PageHeader extends StatelessWidget {
  const PageHeader({
    super.key,
    required this.title,
    this.subtitle,
    this.actions = const [],
    this.eyebrow,
  });

  final String title;
  final String? subtitle;
  final String? eyebrow;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Padding(
      padding: const EdgeInsets.fromLTRB(32, 30, 32, 18),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (eyebrow != null) ...[
                  Text(
                    eyebrow!.toUpperCase(),
                    style: type.body.xs.copyWith(
                      color: colors.primary,
                      fontWeight: FontWeight.w600,
                      letterSpacing: 2.2,
                    ),
                  ),
                  const SizedBox(height: 10),
                ],
                Text(
                  title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: type.display.xl3.copyWith(
                    color: colors.foreground,
                    fontWeight: FontWeight.w600,
                    letterSpacing: -0.6,
                    height: 1.12,
                  ),
                ),
                if (subtitle != null) ...[
                  const SizedBox(height: 8),
                  Text(
                    subtitle!,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: type.body.md.copyWith(
                      color: colors.mutedForeground,
                      height: 1.5,
                    ),
                  ),
                ],
              ],
            ),
          ),
          if (actions.isNotEmpty) ...[
            const SizedBox(width: 20),
            Wrap(spacing: 8, runSpacing: 8, children: actions),
          ],
        ],
      ),
    );
  }
}

/// 小节标题，附带可选的右侧操作。
class SectionTitle extends StatelessWidget {
  const SectionTitle({
    super.key,
    required this.title,
    this.caption,
    this.action,
  });

  final String title;
  final String? caption;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 4,
            height: 18,
            margin: const EdgeInsets.only(right: 12),
            decoration: BoxDecoration(
              color: colors.primary,
              borderRadius: const BorderRadius.all(Radius.circular(2)),
            ),
          ),
          Text(
            title,
            style: type.display.xs.copyWith(
              color: colors.foreground,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (caption != null) ...[
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                caption!,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: type.body.xs.copyWith(color: colors.mutedForeground),
              ),
            ),
          ] else
            const Spacer(),
          ?action,
        ],
      ),
    );
  }
}

/// 空状态：图标、标题、说明与可选操作。
class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    this.description,
    this.action,
  });

  final IconData icon;
  final String title;
  final String? description;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 48),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 58,
              height: 58,
              decoration: BoxDecoration(
                color: colors.primary.withValues(alpha: .12),
                borderRadius: const BorderRadius.all(Radius.circular(18)),
              ),
              child: Icon(icon, size: 26, color: colors.primary),
            ),
            const SizedBox(height: 18),
            Text(
              title,
              textAlign: TextAlign.center,
              style: type.display.xs.copyWith(
                color: colors.foreground,
                fontWeight: FontWeight.w600,
              ),
            ),
            if (description != null) ...[
              const SizedBox(height: 8),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Text(
                  description!,
                  textAlign: TextAlign.center,
                  style: type.body.sm.copyWith(
                    color: colors.mutedForeground,
                    height: 1.55,
                  ),
                ),
              ),
            ],
            if (action != null) ...[const SizedBox(height: 20), action!],
          ],
        ),
      ),
    );
  }
}

/// 载入状态。
class LoadingState extends StatelessWidget {
  const LoadingState({super.key, this.label = '正在载入'});

  final String label;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 56),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const FCircularProgress(size: FCircularProgressSizeVariant.md),
            const SizedBox(height: 14),
            Text(
              label,
              style: context.theme.typography.body.sm.copyWith(
                color: colors.mutedForeground,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// 错误状态，带重试。
class ErrorState extends StatelessWidget {
  const ErrorState({
    super.key,
    required this.message,
    this.onRetry,
    this.icon = FLucideIcons.cloudOff,
  });

  final String message;
  final VoidCallback? onRetry;
  final IconData icon;

  @override
  Widget build(BuildContext context) => EmptyState(
    icon: icon,
    title: '暂时没有内容',
    description: message,
    action: onRetry == null
        ? null
        : FButton(
            variant: FButtonVariant.outline,
            size: FButtonSizeVariant.sm,
            mainAxisSize: MainAxisSize.min,
            prefix: const Icon(FLucideIcons.refreshCw, size: 15),
            onPress: onRetry,
            child: const Text('重试'),
          ),
  );
}

/// 可选择的小标签（频道、文件夹等）。
class FilterPill extends StatelessWidget {
  const FilterPill({
    super.key,
    required this.label,
    required this.selected,
    required this.onPress,
    this.trailing,
    this.icon,
  });

  final String label;
  final bool selected;
  final VoidCallback onPress;
  final Widget? trailing;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return HoverLift(
      scale: 1.02,
      onTap: onPress,
      borderRadius: const BorderRadius.all(Radius.circular(999)),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOutCubic,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: selected
              ? colors.primary.withValues(alpha: .16)
              : colors.secondary.withValues(alpha: .6),
          borderRadius: const BorderRadius.all(Radius.circular(999)),
          border: Border.all(
            color: selected
                ? colors.primary.withValues(alpha: .55)
                : colors.border,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              Icon(
                icon,
                size: 14,
                color: selected ? colors.primary : colors.mutedForeground,
              ),
              const SizedBox(width: 6),
            ],
            Text(
              label,
              style: type.body.xs.copyWith(
                color: selected ? colors.primary : colors.mutedForeground,
                fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
              ),
            ),
            if (trailing != null) ...[const SizedBox(width: 6), trailing!],
          ],
        ),
      ),
    );
  }
}

/// 统一的页面内容容器：限制最大宽度，避免超宽屏阅读困难。
class PageBody extends StatelessWidget {
  const PageBody({super.key, required this.child, this.maxWidth = 1280});

  final Widget child;
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.topLeft,
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: maxWidth),
        child: child,
      ),
    );
  }
}
