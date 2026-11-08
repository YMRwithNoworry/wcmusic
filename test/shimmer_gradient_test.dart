import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/ui/core/shimmer_gradient.dart';

void main() {
  testWidgets('settles when accessibility disables animations', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: ShimmerGradient(child: Text('Playing')),
        ),
      ),
    );

    await tester.pumpAndSettle();

    expect(find.text('Playing'), findsOneWidget);
  });
}
