import 'dart:convert';
import 'dart:io';

class FeedbackRequest {
  const FeedbackRequest({
    required this.issueType,
    required this.platform,
    required this.description,
    this.email = '',
  });

  final String issueType;
  final String platform;
  final String description;
  final String email;

  Map<String, String> toJson() => {
    '_subject': 'WCMusic 软件内问题反馈',
    '_template': 'table',
    '_captcha': 'false',
    '问题类型': issueType,
    '使用平台': platform,
    'email': email,
    '问题描述': description,
  };
}

abstract interface class FeedbackService {
  Future<void> submit(FeedbackRequest feedback);
}

class FormSubmitFeedbackService implements FeedbackService {
  FormSubmitFeedbackService({
    Uri? endpoint,
    HttpClient Function()? clientFactory,
  }) : endpoint =
           endpoint ??
           Uri.parse('https://formsubmit.co/ajax/paojiao134@outlook.com'),
       _clientFactory = clientFactory ?? HttpClient.new;

  final Uri endpoint;
  final HttpClient Function() _clientFactory;

  @override
  Future<void> submit(FeedbackRequest feedback) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 12);
    try {
      final request = await client.postUrl(endpoint);
      request.headers.contentType = ContentType.json;
      request.headers.set(HttpHeaders.acceptHeader, 'application/json');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      request.write(jsonEncode(feedback.toJson()));
      final response = await request.close().timeout(
        const Duration(seconds: 20),
      );
      await response.drain<void>();
      if (response.statusCode < 200 || response.statusCode >= 300) {
        throw HttpException('反馈服务返回 ${response.statusCode}', uri: endpoint);
      }
    } finally {
      client.close(force: true);
    }
  }
}
