import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/feedback_service.dart';

void main() {
  test('submits feedback to the same mail delivery endpoint format', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    Map<String, dynamic>? payload;
    String? accept;
    final requestFuture = server.first.then((request) async {
      accept = request.headers.value(HttpHeaders.acceptHeader);
      payload = jsonDecode(await utf8.decoder.bind(request).join());
      request.response.headers.contentType = ContentType.json;
      request.response.write(jsonEncode({'success': 'true'}));
      await request.response.close();
    });
    final service = FormSubmitFeedbackService(
      endpoint: Uri.parse('http://127.0.0.1:${server.port}/feedback'),
    );

    await service.submit(
      const FeedbackRequest(
        issueType: '播放问题',
        platform: 'Windows 11',
        email: 'user@example.com',
        description: '播放歌曲时无法获取完整的歌曲地址。',
      ),
    );
    await requestFuture;

    expect(accept, 'application/json');
    expect(payload?['_subject'], 'WCMusic 软件内问题反馈');
    expect(payload?['问题类型'], '播放问题');
    expect(payload?['使用平台'], 'Windows 11');
    expect(payload?['email'], 'user@example.com');
    expect(payload?['问题描述'], '播放歌曲时无法获取完整的歌曲地址。');
  });
}
