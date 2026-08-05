import '../models/track.dart';

abstract interface class SourceRepository {
  Future<List<SourceScript>> loadSources();
  Future<SourceScript> importScript(String rawScript);
  Future<void> deleteSource(String id);
  Future<String> resolveUrl(Track track, {String quality = '320k'});
}
