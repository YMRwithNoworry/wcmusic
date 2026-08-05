import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';

import 'test_support.dart';

void main() {
  test('controls playback position, volume, and mute restoration', () async {
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(viewModel.tracks.first);

    await viewModel.seek(const Duration(minutes: 20).inMilliseconds.toDouble());
    expect(player.soughtPosition, viewModel.tracks.first.duration);

    await viewModel.setVolume(.35);
    expect(player.setVolumeValue, .35);
    await viewModel.toggleMute();
    expect(player.setVolumeValue, 0);
    await viewModel.toggleMute();
    expect(player.setVolumeValue, .35);
  });

  test('randomly selects at most four platform playlists', () async {
    final playlists = List.generate(
      6,
      (index) => PlatformPlaylist(
        id: '$index',
        name: '歌单 $index',
        artworkUri: 'https://image.example/$index.jpg',
        url: 'https://music.example/$index',
        platform: '测试平台',
      ),
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(const [], playlists),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    await viewModel.refreshPlatformPlaylists();

    expect(viewModel.platformPlaylists, hasLength(4));
    expect(
      viewModel.platformPlaylists.map((item) => item.id).toSet(),
      hasLength(4),
    );
  });

  test('updates the close-to-tray background playback setting', () async {
    final lifecycle = FakeWindowLifecycleService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      windowLifecycleService: lifecycle,
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    expect(viewModel.backgroundPlayback, isTrue);
    await viewModel.setBackgroundPlayback(false);

    expect(viewModel.backgroundPlayback, isFalse);
    expect(lifecycle.closeToTray, isFalse);
  });
}
