import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/playback_quality.dart';
import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import '../widgets/artwork.dart';
import '../widgets/hover.dart';
import 'controls.dart';

/// 底部播放条。
///
/// 只订阅“当前曲目 / 播放状态 / 模式 / 音量”这些低频状态；进度由 [SeekBar]
/// 自己监听高频通道，避免整条栏每 100ms 重建一次。左侧歌曲信息可点击，打开
/// 聚焦播放的歌词页。
class PlayerBar extends StatelessWidget {
  const PlayerBar({super.key, required this.onOpenNowPlaying});

  final VoidCallback onOpenNowPlaying;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final track = context.select<PlayerViewModel, Track?>(
      (model) => model.current,
    );
    if (track == null) {
      return const _IdlePlayerBar();
    }
    final isPlaying = context.select<PlayerViewModel, bool>(
      (model) => model.isPlaying,
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final compact = width < 880;
        final showVolume = width >= 1080;
        final showExtras = width >= 720;
        return DecoratedBox(
          decoration: BoxDecoration(
            color: colors.card,
            border: Border(top: BorderSide(color: colors.border)),
          ),
          child: SizedBox(
            height: 90,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 18),
              child: Row(
                children: [
                  _NowPlayingInfo(
                    track: track,
                    isPlaying: isPlaying,
                    width: compact ? 240 : 300,
                    onTap: onOpenNowPlaying,
                  ),
                  Expanded(
                    child: Center(
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 560),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 16),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              const TransportControls(compact: true),
                              const SizedBox(height: 2),
                              const SeekBar(compact: true),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  SizedBox(
                    // 音量固定 190 + 三个按钮 + 间距，宽度给足，避免窄窗口溢出。
                    width: compact ? 170 : (showVolume ? 312 : 130),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        if (showExtras) ...[
                          // 下载按钮旁边依次是歌词与播放顺序切换。
                          const LyricsButton(),
                          const SizedBox(width: 2),
                          const DownloadButton(),
                          const SizedBox(width: 2),
                          const PlaybackModeButton(),
                        ],
                        if (showVolume) ...[
                          const SizedBox(width: 8),
                          const VolumeControl(width: 190),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

class _IdlePlayerBar extends StatelessWidget {
  const _IdlePlayerBar();

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.card,
        border: Border(top: BorderSide(color: colors.border)),
      ),
      child: SizedBox(
        height: 62,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 22),
          child: Row(
            children: [
              Icon(
                FLucideIcons.music2,
                size: 16,
                color: colors.mutedForeground,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  '还没有正在播放的歌曲 · 在发现页挑一首开始',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: type.body.xs.copyWith(color: colors.mutedForeground),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _NowPlayingInfo extends StatelessWidget {
  const _NowPlayingInfo({
    required this.track,
    required this.isPlaying,
    required this.width,
    required this.onTap,
  });

  final Track track;
  final bool isPlaying;
  final double width;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return SizedBox(
      width: width,
      child: HoverLift(
        scale: 1.005,
        onTap: onTap,
        borderRadius: const BorderRadius.all(Radius.circular(12)),
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 6),
          child: Row(
            children: [
              ArtworkView(
                seed: track.id,
                url: track.artworkUri,
                size: 54,
                radius: 10,
                playing: isPlaying,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      track.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.sm.copyWith(
                        color: colors.foreground,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      track.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: type.body.xs.copyWith(
                        color: colors.mutedForeground,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// 桌面歌词开关：放在下载按钮旁边，一键显示 / 隐藏桌面歌词。
///
/// 图标沿用 listEnd（文本行 + 音符），开启时高亮。平台不支持时按钮置灰；
/// 设置页里也有同一个开关。
class LyricsButton extends StatelessWidget {
  const LyricsButton({super.key});

  @override
  Widget build(BuildContext context) {
    final enabled = context.select<PlayerViewModel, bool>(
      (model) => model.floatingLyricsEnabled,
    );
    final supported = context.select<PlayerViewModel, bool>(
      (model) => model.floatingLyricsService.isSupported,
    );
    return RoundIconButton(
      key: const ValueKey('floatingLyricsToggle'),
      icon: FLucideIcons.listEnd,
      tooltip: !supported
          ? '当前平台不支持桌面歌词'
          : enabled
          ? '关闭桌面歌词'
          : '开启桌面歌词',
      selected: enabled,
      size: 34,
      iconSize: 17,
      onPress: supported
          ? () => context.read<PlayerViewModel>().setFloatingLyrics(!enabled)
          : null,
    );
  }
}

/// 下载当前曲目，可按音质选择。
class DownloadButton extends StatelessWidget {
  const DownloadButton({super.key});

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    if (model.isDownloading) {
      return Padding(
        padding: const EdgeInsets.all(8),
        child: SizedBox.square(
          dimension: 18,
          child: FCircularProgress(
            size: FCircularProgressSizeVariant.xs,
            semanticsLabel: '正在下载',
          ),
        ),
      );
    }
    final track = model.current;
    final supportsQuality =
        track != null &&
        track.source != TrackSource.local &&
        (track.sourceId?.isNotEmpty ?? false);
    return FPopoverMenu(
      menu: [
        FItemGroup(
          children: [
            if (supportsQuality)
              for (final quality in PlaybackQuality.values)
                FItem(
                  title: Text('下载 · ${quality.label}'),
                  prefix: const Icon(FLucideIcons.download, size: 16),
                  onPress: () => model.downloadCurrentTrack(quality),
                )
            else
              FItem(
                title: const Text('下载 · 原始音质'),
                prefix: const Icon(FLucideIcons.download, size: 16),
                onPress: () => model.downloadCurrentTrack(),
              ),
          ],
        ),
      ],
      builder: (context, controller, child) => RoundIconButton(
        icon: FLucideIcons.download,
        tooltip: '下载当前歌曲',
        size: 34,
        iconSize: 17,
        onPress: track == null ? null : controller.toggle,
      ),
    );
  }
}
