import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../api/grpc_client.dart';
import '../api/repositories/auth_repository.dart';
import '../api/repositories/space_repository.dart';
import '../api/repositories/message_repository.dart';

part 'api_providers.g.dart';

/// Global gRPC client instance
@Riverpod(keepAlive: true)
SpacePandaGrpcClient grpcClient(GrpcClientRef ref) {
  final client = SpacePandaGrpcClient();

  // Close the channel when the provider is disposed
  ref.onDispose(() {
    client.close();
  });

  return client;
}

/// Authentication repository
@riverpod
AuthRepository authRepository(AuthRepositoryRef ref) {
  final client = ref.watch(grpcClientProvider);
  return AuthRepository(client);
}

/// Space repository
@riverpod
SpaceRepository spaceRepository(SpaceRepositoryRef ref) {
  final client = ref.watch(grpcClientProvider);
  return SpaceRepository(client);
}

/// Message repository
@riverpod
MessageRepository messageRepository(MessageRepositoryRef ref) {
  final client = ref.watch(grpcClientProvider);
  return MessageRepository(client);
}

/// Session token state - stores the current user's session token
@Riverpod(keepAlive: true)
class SessionToken extends _$SessionToken {
  @override
  String? build() {
    debugPrint('SessionToken.build() called - initializing to null');
    return null;
  }

  void setToken(String token) {
    final preview = token.length >= 8 ? token.substring(0, 8) : token;
    debugPrint('SessionToken.setToken() - Setting token: $preview...');
    state = token;
    final statePreview =
        state != null && state!.length >= 8 ? state!.substring(0, 8) : state;
    debugPrint(
        'SessionToken.setToken() - State updated, current state: $statePreview');
  }

  void clearToken() {
    debugPrint('SessionToken.clearToken() - Clearing token');
    state = null;
  }
}

/// Check if user is authenticated
@riverpod
bool isAuthenticated(IsAuthenticatedRef ref) {
  final token = ref.watch(sessionTokenProvider);
  final preview = token != null && token.length >= 8
      ? '${token.substring(0, 8)}...'
      : token;
  debugPrint('isAuthenticated() - token: ${preview ?? "null"}');
  return token != null;
}
