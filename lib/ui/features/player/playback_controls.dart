import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'player_view_model.dart';

class PlaybackProgress extends StatelessWidget {
  const PlaybackProgress({
    super.key,
    this.showTimes = true,
    this.compact = false,
  });

  final bool showTimes;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final duration = viewModel.playbackDuration;
    final maximum = duration.inMilliseconds;
    final position = viewModel.position.inMilliseconds
        .clamp(0, maximum <= 0 ? 1 : maximum)
        .toDouble();
    final slider = SliderTheme(
      data: SliderTheme.of(context).copyWith(
        trackHeight: compact ? 2 : 4,
        thumbShape: RoundSliderThumbShape(enabledThumbRadius: compact ? 5 : 7),
        overlayShape: RoundSliderOverlayShape(overlayRadius: compact ? 12 : 16),
      ),
      child: Slider(
        value: position,
        max: maximum <= 0 ? 1 : maximum.toDouble(),
        onChanged: maximum <= 0 ? null : viewModel.seek,
      ),
    );
    if (!showTimes) return slider;
    return Column(
      children: [
        slider,
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [Text(_time(viewModel.position)), Text(_time(duration))],
          ),
        ),
      ],
    );
  }

  String _time(Duration value) {
    final safe = value.isNegative ? Duration.zero : value;
    return '${safe.inMinutes}:${(safe.inSeconds % 60).toString().padLeft(2, '0')}';
  }
}

class VolumeControl extends StatelessWidget {
  const VolumeControl({super.key, this.showLabel = true});

  final bool showLabel;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          onPressed: viewModel.toggleMute,
          icon: Icon(_icon(viewModel.volume)),
          tooltip: viewModel.volume <= .001 ? '取消静音' : '静音',
        ),
        Expanded(
          child: Slider(
            value: viewModel.volume,
            onChanged: viewModel.setVolume,
          ),
        ),
        if (showLabel)
          SizedBox(
            width: 42,
            child: Text(
              '${(viewModel.volume * 100).round()}%',
              textAlign: TextAlign.end,
            ),
          ),
      ],
    );
  }

  IconData _icon(double volume) {
    if (volume <= .001) return Icons.volume_off;
    if (volume < .5) return Icons.volume_down;
    return Icons.volume_up;
  }
}

class DownloadButton extends StatelessWidget {
  const DownloadButton({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return IconButton(
      onPressed: viewModel.isDownloading
          ? null
          : viewModel.downloadCurrentTrack,
      icon: viewModel.isDownloading
          ? SizedBox.square(
              dimension: 20,
              child: CircularProgressIndicator(
                value: viewModel.downloadProgress,
                strokeWidth: 2,
              ),
            )
          : const Icon(Icons.download_outlined),
      tooltip: '下载到本地',
    );
  }
}
