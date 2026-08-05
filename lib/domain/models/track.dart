enum TrackSource { local, kw, kg, tx, wy, mg, custom }

class Track {
  const Track({
    required this.id,
    required this.title,
    required this.artist,
    required this.album,
    required this.duration,
    required this.uri,
    this.artworkUri,
    this.source = TrackSource.local,
    this.sourceId,
    this.quality,
    this.releaseDate,
    this.isFavorite = false,
  });

  final String id;
  final String title;
  final String artist;
  final String album;
  final Duration duration;
  final String uri;
  final String? artworkUri;
  final TrackSource source;
  final String? sourceId;
  final String? quality;
  final DateTime? releaseDate;
  final bool isFavorite;

  Track copyWith({String? title, bool? isFavorite}) => Track(
    id: id,
    title: title ?? this.title,
    artist: artist,
    album: album,
    duration: duration,
    uri: uri,
    artworkUri: artworkUri,
    source: source,
    sourceId: sourceId,
    quality: quality,
    releaseDate: releaseDate,
    isFavorite: isFavorite ?? this.isFavorite,
  );
}

class Playlist {
  const Playlist({
    required this.id,
    required this.name,
    required this.tracks,
    this.artworkUri,
    this.externalUrl,
    this.platform,
  });

  final String id;
  final String name;
  final List<Track> tracks;
  final String? artworkUri;
  final String? externalUrl;
  final String? platform;

  bool get isPlatformFavorite => externalUrl?.isNotEmpty ?? false;
}

class PlatformPlaylist {
  const PlatformPlaylist({
    required this.id,
    required this.name,
    required this.artworkUri,
    required this.url,
    required this.platform,
  });

  final String id;
  final String name;
  final String artworkUri;
  final String url;
  final String platform;
}

class SourceScript {
  const SourceScript({
    required this.id,
    required this.name,
    required this.version,
    required this.author,
    required this.description,
    required this.sourceKeys,
    required this.rawScript,
  });

  final String id;
  final String name;
  final String version;
  final String author;
  final String description;
  final List<String> sourceKeys;
  final String rawScript;
}
