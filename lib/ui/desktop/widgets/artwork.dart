import 'package:flutter/widgets.dart';

import '../../core/organic_artwork.dart';
import '../../core/remote_artwork.dart';

/// 统一封面：有远程图走带缓存的 [RemoteArtwork]，否则用种子生成渐变封面。
class ArtworkView extends StatelessWidget {
  const ArtworkView({
    super.key,
    required this.seed,
    this.url,
    this.size = 48,
    this.radius = 10,
    this.playing = false,
  });

  final String seed;
  final String? url;
  final double size;
  final double radius;
  final bool playing;

  @override
  Widget build(BuildContext context) {
    final artwork = (url == null || url!.isEmpty)
        ? OrganicArtwork(seed: seed, size: double.infinity, playing: playing)
        : RemoteArtwork(url: url!, seed: seed);
    final clipped = RepaintBoundary(
      child: ClipRRect(
        borderRadius: BorderRadius.circular(radius),
        child: artwork,
      ),
    );
    if (size.isInfinite) return SizedBox.expand(child: clipped);
    return SizedBox.square(dimension: size, child: clipped);
  }
}
