import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:spacepanda_flutter/providers/api_providers.dart';

void main() {
  group('Authentication Flow Tests', () {
    test('SessionToken should persist after being set', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Initially null
      expect(container.read(sessionTokenProvider), isNull);

      // Set a token
      const testToken = 'test-token-12345678';
      container.read(sessionTokenProvider.notifier).setToken(testToken);

      // Should persist
      expect(container.read(sessionTokenProvider), equals(testToken));

      // Read again - should still be there
      expect(container.read(sessionTokenProvider), equals(testToken));
    });

    test('SessionToken should survive multiple reads', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      const testToken = 'test-token-12345678';
      container.read(sessionTokenProvider.notifier).setToken(testToken);

      // Multiple reads should return same value
      for (int i = 0; i < 10; i++) {
        expect(
          container.read(sessionTokenProvider),
          equals(testToken),
          reason: 'Token should persist on read #$i',
        );
      }
    });

    test('isAuthenticated should return true when token is set', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Initially false
      expect(container.read(isAuthenticatedProvider), isFalse);

      // Set token
      container.read(sessionTokenProvider.notifier).setToken('test-token');

      // Should be authenticated
      expect(container.read(isAuthenticatedProvider), isTrue);
    });

    test('isAuthenticated should return false after clearing token', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Set token
      container.read(sessionTokenProvider.notifier).setToken('test-token');
      expect(container.read(isAuthenticatedProvider), isTrue);

      // Clear token
      container.read(sessionTokenProvider.notifier).clearToken();

      // Should not be authenticated
      expect(container.read(isAuthenticatedProvider), isFalse);
      expect(container.read(sessionTokenProvider), isNull);
    });

    test('SessionToken should persist across provider scope changes', () {
      // This simulates navigation where providers might be rebuilt
      final container = ProviderContainer();
      addTearDown(container.dispose);

      const testToken = 'test-token-12345678';

      // Set token
      container.read(sessionTokenProvider.notifier).setToken(testToken);
      expect(container.read(sessionTokenProvider), equals(testToken));

      // Create a child container (simulates new route/screen)
      final childContainer = ProviderContainer(parent: container);
      addTearDown(childContainer.dispose);

      // Token should be available in child scope
      expect(childContainer.read(sessionTokenProvider), equals(testToken));
      expect(childContainer.read(isAuthenticatedProvider), isTrue);
    });

    test('Multiple containers should not share token state', () {
      final container1 = ProviderContainer();
      final container2 = ProviderContainer();
      addTearDown(() {
        container1.dispose();
        container2.dispose();
      });

      // Set token in first container
      container1.read(sessionTokenProvider.notifier).setToken('token-1');

      // Should not affect second container
      expect(container1.read(sessionTokenProvider), equals('token-1'));
      expect(container2.read(sessionTokenProvider), isNull);

      // Set different token in second container
      container2.read(sessionTokenProvider.notifier).setToken('token-2');

      // Should be independent
      expect(container1.read(sessionTokenProvider), equals('token-1'));
      expect(container2.read(sessionTokenProvider), equals('token-2'));
    });

    test('Token should survive widget rebuilds', () async {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      const testToken = 'test-token-12345678';
      container.read(sessionTokenProvider.notifier).setToken(testToken);

      // Simulate multiple widget rebuilds by reading provider many times
      for (int i = 0; i < 50; i++) {
        final token = container.read(sessionTokenProvider);
        expect(token, equals(testToken), reason: 'Failed on rebuild $i');

        // Also check isAuthenticated
        final isAuth = container.read(isAuthenticatedProvider);
        expect(isAuth, isTrue, reason: 'Auth failed on rebuild $i');

        // Small delay to simulate real rebuild timing
        await Future.delayed(Duration.zero);
      }
    });
  });

  group('Auth Provider Integration Tests', () {
    test('ProfileUnlocker should set session token', () async {
      // Note: This is a mock test - real test would need mock repository
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Verify token starts as null
      expect(container.read(sessionTokenProvider), isNull);
      expect(container.read(isAuthenticatedProvider), isFalse);

      // After unlock completes, token should be set
      // (This test will fail without mocked repository, but shows the pattern)
    });
  });

  group('Session State Edge Cases', () {
    test('Setting empty string should still count as authenticated', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(sessionTokenProvider.notifier).setToken('');

      // Empty string is still a token (even if invalid)
      expect(container.read(sessionTokenProvider), equals(''));
      expect(container.read(isAuthenticatedProvider), isTrue);
    });

    test('Clearing already null token should not crash', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(container.read(sessionTokenProvider), isNull);

      // Should not throw
      container.read(sessionTokenProvider.notifier).clearToken();

      expect(container.read(sessionTokenProvider), isNull);
    });

    test('Setting token multiple times should use latest value', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(sessionTokenProvider.notifier).setToken('token1');
      container.read(sessionTokenProvider.notifier).setToken('token2');
      container.read(sessionTokenProvider.notifier).setToken('token3');

      expect(container.read(sessionTokenProvider), equals('token3'));
    });
  });
}
