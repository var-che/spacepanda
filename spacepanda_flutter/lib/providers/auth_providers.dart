import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'api_providers.dart';

part 'auth_providers.g.dart';

/// Create a new profile and authenticate
@riverpod
class ProfileCreator extends _$ProfileCreator {
  @override
  FutureOr<String?> build() => null;

  Future<String> createProfile({
    required String password,
    String? username,
  }) async {
    state = const AsyncValue.loading();

    state = await AsyncValue.guard(() async {
      final repository = ref.read(authRepositoryProvider);
      final token = await repository.createProfile(
        password: password,
        username: username,
      );

      // Store the session token
      ref.read(sessionTokenProvider.notifier).setToken(token);

      return token;
    });

    // If there was an error, rethrow it
    if (state.hasError) {
      throw state.error!;
    }

    return state.requireValue!;
  }
}

/// Unlock an existing profile
@riverpod
class ProfileUnlocker extends _$ProfileUnlocker {
  @override
  FutureOr<String?> build() => null;

  Future<String> unlock({
    required String password,
    String? username,
  }) async {
    state = const AsyncValue.loading();

    state = await AsyncValue.guard(() async {
      final repository = ref.read(authRepositoryProvider);
      final token = await repository.unlock(
        password: password,
        username: username,
      );

      final preview = token.length >= 8 ? token.substring(0, 8) : token;
      print('ProfileUnlocker: Got token from repository: $preview...');

      // Store the session token
      ref.read(sessionTokenProvider.notifier).setToken(token);

      print('ProfileUnlocker: Token stored, verifying...');
      final storedToken = ref.read(sessionTokenProvider);
      final storedPreview =
          storedToken != null && storedToken.length >= 8
              ? storedToken.substring(0, 8)
              : storedToken;
      print(
          'ProfileUnlocker: Verification - stored token: ${storedToken != null ? "$storedPreview..." : "NULL!"}');

      return token;
    });

    // If there was an error, rethrow it
    if (state.hasError) {
      throw state.error!;
    }

    return state.requireValue!;
  }
}

/// Lock (logout) the current session
@riverpod
class SessionLocker extends _$SessionLocker {
  @override
  FutureOr<void> build() => null;

  Future<void> lock() async {
    final sessionToken = ref.read(sessionTokenProvider);
    if (sessionToken == null) return;

    state = const AsyncValue.loading();

    state = await AsyncValue.guard(() async {
      final repository = ref.read(authRepositoryProvider);
      await repository.lock(sessionToken);

      // Clear the session token
      ref.read(sessionTokenProvider.notifier).clearToken();
    });
  }
}
