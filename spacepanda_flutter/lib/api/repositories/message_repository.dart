import 'package:grpc/grpc.dart';
import '../grpc_client.dart';
import '../../generated/spacepanda.pbgrpc.dart';
import '../../shared/models/models.dart' as app;

class MessageRepository {
  final SpacePandaGrpcClient _client;

  MessageRepository(this._client);

  /// Get messages from a channel
  Future<List<app.Message>> getMessages({
    required String sessionToken,
    required String channelId,
    int limit = 50,
  }) async {
    try {
      final response = await _client.messages.getMessages(
        GetMessagesRequest(
          sessionToken: sessionToken,
          channelId: channelId,
          limit: limit,
        ),
      );
      return response.messages.map(_mapMessage).toList();
    } on GrpcError catch (e) {
      throw MessageException('Failed to get messages: ${e.message}');
    }
  }

  /// Send a message to a channel
  Future<app.Message> sendMessage({
    required String sessionToken,
    required String channelId,
    required String content,
    String? replyTo,
  }) async {
    try {
      final response = await _client.messages.sendMessage(
        SendMessageRequest(
          sessionToken: sessionToken,
          channelId: channelId,
          content: content,
        ),
      );
      return _mapMessage(response);
    } on GrpcError catch (e) {
      throw MessageException('Failed to send message: ${e.message}');
    }
  }

  /// Stream messages in real-time
  Stream<app.Message> streamMessages({
    required String sessionToken,
    required String channelId,
  }) async* {
    try {
      final stream = _client.messages.streamMessages(
        StreamMessagesRequest(
          sessionToken: sessionToken,
          channelId: channelId,
        ),
      );

      await for (final message in stream) {
        yield _mapMessage(message);
      }
    } on GrpcError catch (e) {
      throw MessageException('Failed to stream messages: ${e.message}');
    }
  }

  // Helper to map proto message to app message
  app.Message _mapMessage(Message protoMessage) {
    return app.Message(
      id: protoMessage.id,
      channelId: protoMessage.channelId,
      senderId: protoMessage.senderId,
      content: protoMessage.content,
      timestamp:
          DateTime.fromMillisecondsSinceEpoch(protoMessage.timestamp.toInt()),
      isE2ee: protoMessage.isE2ee,
    );
  }
}

class MessageException implements Exception {
  final String message;
  MessageException(this.message);

  @override
  String toString() => message;
}
