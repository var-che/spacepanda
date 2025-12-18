import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:spacepanda_flutter/providers/api_providers.dart';

/// Tests that verify state persistence across navigation
void main() {
  group('Navigation State Persistence', () {
    testWidgets('Session token should persist after navigation', (tester) async {
      final container = ProviderContainer();
      
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: Consumer(
                builder: (context, ref, child) {
                  final token = ref.watch(sessionTokenProvider);
                  return Column(
                    children: [
                      Text('Token: ${token ?? "null"}'),
                      ElevatedButton(
                        onPressed: () {
                          ref.read(sessionTokenProvider.notifier).setToken('test-token');
                        },
                        child: const Text('Set Token'),
                      ),
                      ElevatedButton(
                        onPressed: () {
                          Navigator.of(context).push(
                            MaterialPageRoute(
                              builder: (_) => const SecondScreen(),
                            ),
                          );
                        },
                        child: const Text('Navigate'),
                      ),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      );

      // Initially null
      expect(find.text('Token: null'), findsOneWidget);

      // Set token
      await tester.tap(find.text('Set Token'));
      await tester.pump();
      expect(find.text('Token: test-token'), findsOneWidget);

      // Navigate to second screen
      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();

      // Token should still be accessible on second screen
      expect(find.text('Second Screen Token: test-token'), findsOneWidget);

      // Navigate back
      await tester.pageBack();
      await tester.pumpAndSettle();

      // Token should still be there
      expect(find.text('Token: test-token'), findsOneWidget);

      container.dispose();
    });

    testWidgets('Session token should persist after pushReplacement', (tester) async {
      final container = ProviderContainer();
      
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: Consumer(
                builder: (context, ref, child) {
                  return ElevatedButton(
                    onPressed: () {
                      // Set token
                      ref.read(sessionTokenProvider.notifier).setToken('auth-token');
                      
                      // Navigate with replacement (like login flow)
                      Navigator.of(context).pushReplacement(
                        MaterialPageRoute(
                          builder: (_) => const SecondScreen(),
                        ),
                      );
                    },
                    child: const Text('Login'),
                  );
                },
              ),
            ),
          ),
        ),
      );

      // Tap login button (sets token and navigates)
      await tester.tap(find.text('Login'));
      await tester.pumpAndSettle();

      // Should be on second screen with token available
      expect(find.text('Second Screen Token: auth-token'), findsOneWidget);

      container.dispose();
    });

    testWidgets('isAuthenticated should remain true after navigation', (tester) async {
      final container = ProviderContainer();
      
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: Consumer(
                builder: (context, ref, child) {
                  final isAuth = ref.watch(isAuthenticatedProvider);
                  return Column(
                    children: [
                      Text('Authenticated: $isAuth'),
                      ElevatedButton(
                        onPressed: () {
                          ref.read(sessionTokenProvider.notifier).setToken('token');
                        },
                        child: const Text('Authenticate'),
                      ),
                      ElevatedButton(
                        onPressed: () {
                          Navigator.of(context).push(
                            MaterialPageRoute(builder: (_) => const AuthCheckScreen()),
                          );
                        },
                        child: const Text('Check Auth'),
                      ),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      );

      // Initially not authenticated
      expect(find.text('Authenticated: false'), findsOneWidget);

      // Authenticate
      await tester.tap(find.text('Authenticate'));
      await tester.pump();
      expect(find.text('Authenticated: true'), findsOneWidget);

      // Navigate to auth check screen
      await tester.tap(find.text('Check Auth'));
      await tester.pumpAndSettle();

      // Should still be authenticated
      expect(find.text('Auth Check: true'), findsOneWidget);

      container.dispose();
    });
  });
}

class SecondScreen extends ConsumerWidget {
  const SecondScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final token = ref.watch(sessionTokenProvider);
    return Scaffold(
      body: Center(
        child: Text('Second Screen Token: ${token ?? "null"}'),
      ),
    );
  }
}

class AuthCheckScreen extends ConsumerWidget {
  const AuthCheckScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isAuth = ref.watch(isAuthenticatedProvider);
    return Scaffold(
      body: Center(
        child: Text('Auth Check: $isAuth'),
      ),
    );
  }
}
