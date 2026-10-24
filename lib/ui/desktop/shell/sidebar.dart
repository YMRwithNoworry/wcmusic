import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import '../nav.dart';
import '../widgets/artwork.dart';
import '../widgets/hover.dart';
import '../widgets/track_list.dart';

const appVersion = String.fromEnvironment('APP_VERSION');

/// 桌面端侧栏：品牌、分组导航、正在播放卡片、版本号。
class DesktopSidebar extends StatelessWidget {
  const DesktopSidebar({
    super.key,
    required this.section,
    required this.onSelect,
    required this.onOpenNowPlaying,
    this.nowPlayingActive = false,
  });

  final AppSection section;
  final ValueChanged<AppSection> onSelect;
  final VoidCallback onOpenNowPlaying;
  final bool nowPlayingActive;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return FSidebar(
      style: .delta(
        constraints: const BoxConstraints.tightFor(width: 238),
        headerPadding: EdgeInsetsGeometryDelta.value(const EdgeInsets.fromLTRB(20, 22, 20, 14)),
        contentPadding: EdgeInsetsGeometryDelta.value(const EdgeInsets.symmetric(vertical: 4)),
        footerPadding: EdgeInsetsGeometryDelta.value(const EdgeInsets.fromLTRB(14, 8, 14, 14)),
      ),
      header: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              ClipRRect(
                borderRadius: const BorderRadius.all(Radius.circular(11)),
                child: Image.asset(
                  'assets/icons/app_icon.png',
                  width: 38,
                  height: 38,
                  filterQuality: FilterQuality.medium,
                ),
              ),
              const SizedBox(width: 11),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'WCMusic',
                      style: type.display.xs.copyWith(
                        color: colors.foreground,
                        fontWeight: FontWeight.w700,
                        letterSpacing: -.2,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      '随时听见喜欢',
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                        fontSize: 11,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _NowPlayingCard(
            active: nowPlayingActive,
            onTap: onOpenNowPlaying,
          ),
          const SizedBox(height: 10),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child: Text(
              appVersion.isEmpty ? '' : 'WCMusic $appVersion',
              style: type.body.xs.copyWith(
                color: colors.mutedForeground.withValues(alpha: .75),
                fontSize: 11,
              ),
            ),
          ),
        ],
      ),
      children: [
        for (final group in const [
          ('浏览', browseSections),
          ('资料库', librarySections),
          ('系统', systemSections),
        ])
          Padding(
            padding: const EdgeInsets.only(top: 10),
            child: FSidebarGroup(
              label: Text(
                group.$1,
                style: type.body.xs.copyWith(
                  color: colors.mutedForeground.withValues(alpha: .7),
                  fontSize: 11,
                  letterSpacing: 1.2,
                ),
              ),
              children: [
                for (final item in group.$2)
                  FSidebarItem(
                    selected: section == item,
                    onPress: () => onSelect(item),
                    icon: Icon(item.icon, size: 17),
                    label: Text(
                      item.label,
                      style: type.body.sm.copyWith(
                        color: section == item
                            ? colors.foreground
                            : colors.mutedForeground,
                        fontWeight: section == item
                            ? FontWeight.w600
                            : FontWeight.w500,
                      ),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

class _NowPlayingCard extends StatelessWidget {
  const _NowPlayingCard({required this.active, required this.onTap});

  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    final model = context.watch<PlayerViewModel>();
    final track = model.current;
    return HoverLift(
      scale: 1.01,
      onTap: track == null ? null : onTap,
      borderRadius: const BorderRadius.all(Radius.circular(12)),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        padding: const EdgeInsets.all(9),
        decoration: BoxDecoration(
          color: active
              ? colors.primary.withValues(alpha: .10)
              : colors.secondary.withValues(alpha: .55),
          borderRadius: const BorderRadius.all(Radius.circular(12)),
          border: Border.all(
            color: active
                ? colors.primary.withValues(alpha: .35)
                : colors.border,
          ),
        ),
        child: Row(
          children: [
            if (track != null)
              ArtworkView(
                seed: track.id,
                url: track.artworkUri,
                size: 36,
                radius: 8,
                playing: model.isPlaying,
              )
            else
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: colors.muted,
                  borderRadius: const BorderRadius.all(Radius.circular(8)),
                ),
                child: Icon(
                  FLucideIcons.music2,
                  size: 15,
                  color: colors.mutedForeground,
                ),
              ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    track?.title ?? '挑一首开始',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: type.body.xs.copyWith(
                      color: colors.foreground,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  if (track != null) ...[
                    const SizedBox(height: 2),
                    Text(
                      track.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                        fontSize: 10.5,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (track != null)
              PlayingBars(playing: model.isPlaying, height: 13),
          ],
        ),
      ),
    );
  }
}

/// 窄窗口下的导航面板，用 Forui 的 sheet 呈现。
Future<AppSection?> showCompactNavigation(
  BuildContext context, {
  required AppSection current,
}) {
  return showFSheet<AppSection>(
    context: context,
    side: FLayout.btt,
    mainAxisMaxRatio: .62,
    builder: (sheetContext) {
      final colors = sheetContext.theme.colors;
      final type = sheetContext.theme.typography;
      return Padding(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Center(
              child: Container(
                width: 38,
                height: 4,
                margin: const EdgeInsets.only(bottom: 14),
                decoration: BoxDecoration(
                  color: colors.border,
                  borderRadius: const BorderRadius.all(Radius.circular(2)),
                ),
              ),
            ),
            Expanded(
              child: ListView(
                children: [
                  for (final item in AppSection.values)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 4),
                      child: FTappable(
                        onPress: () => Navigator.of(sheetContext).pop(item),
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 14,
                            vertical: 12,
                          ),
                          decoration: BoxDecoration(
                            color: item == current
                                ? colors.primary.withValues(alpha: .12)
                                : const Color(0x00000000),
                            borderRadius: const BorderRadius.all(
                              Radius.circular(12),
                            ),
                          ),
                          child: Row(
                            children: [
                              Icon(
                                item.icon,
                                size: 18,
                                color: item == current
                                    ? colors.primary
                                    : colors.mutedForeground,
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      item.label,
                                      style: type.body.sm.copyWith(
                                        color: item == current
                                            ? colors.primary
                                            : colors.foreground,
                                        fontWeight: FontWeight.w600,
                                      ),
                                    ),
                                    Text(
                                      item.caption,
                                      style: type.body.xs.copyWith(
                                        color: colors.mutedForeground,
                                        fontSize: 11,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              if (item == current)
                                Icon(
                                  FLucideIcons.check,
                                  size: 16,
                                  color: colors.primary,
                                ),
                            ],
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      );
    },
  );
}

/// 紧凑模式下的底部导航。
class CompactBottomNavigation extends StatelessWidget {
  const CompactBottomNavigation({
    super.key,
    required this.section,
    required this.onSelect,
  });

  final AppSection section;
  final ValueChanged<AppSection> onSelect;

  @override
  Widget build(BuildContext context) {
    final index = compactSections.indexOf(section);
    return FBottomNavigationBar(
      index: index < 0 ? 0 : index,
      onChange: (value) => onSelect(compactSections[value]),
      children: [
        for (final item in compactSections)
          FBottomNavigationBarItem(
            icon: Icon(item.icon),
            label: Text(item.shortLabel),
          ),
      ],
    );
  }
}

/// 供壳层判断当前是否有歌曲。
bool hasCurrentTrack(BuildContext context) =>
    context.select<PlayerViewModel, Track?>((model) => model.current) != null;
