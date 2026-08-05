import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/organic_artwork.dart';
import 'playback_mode_button.dart';
import 'playback_controls.dart';
import 'player_view_model.dart';

class PlayerBar extends StatelessWidget {
  const PlayerBar({super.key, required this.onOpen});

  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final track = viewModel.current;
    if (track == null) return const SizedBox.shrink();
    final compact = MediaQuery.sizeOf(context).width < 560;
    return Material(
      color: Theme.of(context).colorScheme.surface,
      child: InkWell(
        onTap: onOpen,
        child: SafeArea(
          top: false,
          bottom: false,
          child: SizedBox(
            height: compact ? 92 : 104,
            child: Column(
              children: [
                Expanded(
                  child: Row(
                    children: [
                      const SizedBox(width: 14),
                      Hero(
                        tag: 'art-${track.id}',
                        child: OrganicArtwork(
                          seed: track.id,
                          size: 56,
                          playing: viewModel.isPlaying,
                          artworkUri: track.artworkUri,
                        ),
                      ),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              track.title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.titleLarge,
                            ),
                            Text(
                              track.artist,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ],
                        ),
                      ),
                      if (!compact) ...[
                        PlaybackModeButton(),
                        const SizedBox(width: 2),
                        IconButton(
                          onPressed: viewModel.playPrevious,
                          icon: const Icon(Icons.skip_previous),
                          tooltip: '上一首',
                        ),
                        const SizedBox(width: 2),
                      ],
                      IconButton.filled(
                        onPressed: viewModel.togglePlayback,
                        icon: AnimatedSwitcher(
                          duration: const Duration(milliseconds: 180),
                          child: Icon(
                            viewModel.isPlaying
                                ? Icons.pause
                                : Icons.play_arrow,
                            key: ValueKey(viewModel.isPlaying),
                          ),
                        ),
                        tooltip: viewModel.isPlaying ? '暂停' : '播放',
                      ),
                      if (!compact) ...[
                        const SizedBox(width: 2),
                        IconButton(
                          onPressed: viewModel.playNext,
                          icon: const Icon(Icons.skip_next),
                          tooltip: '下一首',
                        ),
                        const SizedBox(width: 2),
                        const DownloadButton(),
                        const SizedBox(
                          width: 132,
                          child: VolumeControl(showLabel: false),
                        ),
                      ],
                      if (compact) ...[
                        PlaybackModeButton(),
                        const SizedBox(width: 6),
                      ],
                      const SizedBox(width: 14),
                    ],
                  ),
                ),
                const SizedBox(
                  height: 28,
                  child: PlaybackProgress(showTimes: false, compact: true),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
