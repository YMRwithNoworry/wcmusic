import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/ui/core/remote_artwork.dart';

void main() {
  test('loads artwork with browser headers after a proxy failure', () async {
    final bytes = List<int>.generate(512, (index) => index % 251);
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      expect(
        request.headers.value(HttpHeaders.userAgentHeader),
        contains('Mozilla/5.0'),
      );
      expect(
        request.headers.value(HttpHeaders.refererHeader),
        'https://music.163.com/',
      );
      request.response.headers.contentType = ContentType('image', 'jpeg');
      request.response.contentLength = bytes.length;
      request.response.add(bytes);
      await request.response.close();
    });
    final loader = ArtworkLoader(proxyResolver: (_) => 'PROXY 127.0.0.1:1');

    final result = await loader.load(
      'http://127.0.0.1:${server.port}/artwork.jpg',
    );

    expect(result, bytes);
  });
}
