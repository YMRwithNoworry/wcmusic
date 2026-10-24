import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:wcmusic/data/services/feedback_service.dart';
import 'package:wcmusic/ui/core/app_theme.dart';
import 'package:wcmusic/ui/desktop/pages/feedback_dialog.dart';

void main() {
  testWidgets('validates and sends feedback from the in-app form', (
    tester,
  ) async {
    final service = _FakeFeedbackService();
    await tester.pumpWidget(
      MaterialApp(
        home: FTheme(
          data: AppTheme.dark,
          child: Scaffold(
            body: FeedbackForm(feedbackService: service),
          ),
        ),
      ),
    );

    final selects = find.byWidgetPredicate((widget) => widget is FSelect<String>);
    await tester.tap(selects.first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('播放问题').last);
    await tester.pumpAndSettle();
    await tester.tap(selects.last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Windows 11').last);
    await tester.pumpAndSettle();

    final emailField = find.descendant(
      of: find.widgetWithText(FTextField, '联系邮箱（选填）'),
      matching: find.byType(EditableText),
    );
    final descriptionField = find.descendant(
      of: find.widgetWithText(FTextField, '问题描述'),
      matching: find.byType(EditableText),
    );
    await tester.enterText(emailField, 'user@example.com');
    await tester.enterText(descriptionField, '播放歌曲时无法获取完整的歌曲地址。');
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
