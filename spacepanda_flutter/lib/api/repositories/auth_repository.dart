import 'dart:io';
import 'package:grpc/grpc.dart';
import '../grpc_client.dart';
import '../../generated/spacepanda.pbgrpc.dart';

class AuthRepository {
  final SpacePandaGrpcClient _client;
  static final _logFile = File('/tmp/spacepanda_flutter_debug.log');

  AuthRepository(this._client);

  void _log(String message) {
    final timestamp = DateTime.now().toIso8601String();
    final logMessage = '[$timestamp] $message\n';
    print(message);
    try {
      _logFile.writeAsStringSync(logMessage, mode: FileMode.append);
    } catch (e) {
      print('Failed to write to log file: $e');
    }
  }

  /// Create a new user profile with password
  Future<String> createProfile({
    required String password,
    String? username,
  }) async {
    try {
      _log(
          'AuthRepository: Creating profile for username: ${username ?? "(default)"}');
      _log('AuthRepository: Making gRPC call to createProfile...');

      final response = await _client.auth.createProfile(
        CreateProfileRequest(
          username: username,
          password: password,
        ),
        options: CallOptions(
          timeout: const Duration(seconds: 30),
        ),
      );

      _log(
          'AuthRepository: Profile created successfully, token: ${response.sessionToken.substring(0, 8)}...');
      return response.sessionToken;
    } on GrpcError catch (e) {
      _log(
          'AuthRepository: GrpcError - code: ${e.code}, message: ${e.message}, codeName: ${e.codeName}');
      throw AuthException('Failed to create profile: ${e.message}');
    } catch (e, stackTrace) {
      _log('AuthRepository: Unexpected error - $e');
      _log('Stack trace: $stackTrace');
      throw AuthException('Connection error: $e');
    }
  }

  /// Unlock an existing profile with password
  Future<String> unlock({
    required String password,
    String? username,
  }) async {
    try {
      _log(
          'AuthRepository: Unlocking profile for username: ${username ?? "(default)"}');
      final response = await _client.auth.unlock(
        UnlockRequest(
          username: username,
          password: password,
        ),
      );
      _log(
          'AuthRepository: Unlock successful, token: ${response.sessionToken.substring(0, 8)}...');
      return response.sessionToken;
    } on GrpcError catch (e) {
      _log(
          'AuthRepository: GrpcError - code: ${e.code}, message: ${e.message}');
      throw AuthException('Failed to unlock: ${e.message}');
    } catch (e, stackTrace) {
      _log('AuthRepository: Unexpected error - $e');
      _log('Stack trace: $stackTrace');
      throw AuthException('Connection error: $e');
    }
  }

  /// Lock (logout) the current session
  Future<void> lock(String sessionToken) async {
    try {
      await _client.auth.lock(
        LockRequest(sessionToken: sessionToken),
      );
    } on GrpcError catch (e) {
      throw AuthException('Failed to lock: ${e.message}');
    }
  }
}

class AuthException implements Exception {
  final String message;
  AuthException(this.message);

  @override
  String toString() => message;
}
