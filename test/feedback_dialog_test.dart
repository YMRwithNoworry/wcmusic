import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/feedback_service.dart';
import 'package:wcmusic/ui/features/settings/feedback_dialog.dart';

void main() {
  testWidgets('validates and sends feedback from the in-app form', (
    tester,
  ) async {
    final service = _FakeFeedbackService();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: FeedbackDialog(feedbackService: service)),
      ),
    );

    await tester.tap(find.byType(DropdownButtonFormField<String>).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('播放问题').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byType(DropdownButtonFormField<String>).last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Windows 11').last);
    await tester.pumpAndSettle();

    final fields = find.byType(TextFormField);
    await tester.enterText(fields.first, 'user@example.com');
    await tester.enterText(fields.last, '播放歌曲时无法获取完整的歌曲地址。');
    await tester.ensureVisible(find.text('发送反馈'));
    await tester.tap(find.text('发送反馈'));
    await tester.pumpAndSettle();

    expect(service.feedback?.issueType, '播放问题');
    expect(service.feedback?.platform, 'Windows 11');
    expect(service.feedback?.email, 'user@example.com');
    expect(find.text('已送出，感谢你帮助 WCMusic 变得更好。'), findsOneWidget);
  });
}

class _FakeFeedbackService implements FeedbackService {
  FeedbackRequest? feedback;

  @override
  Future<void> submit(FeedbackRequest feedback) async {
    this.feedback = feedback;
  }
}
