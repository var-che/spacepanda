import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../shared/models/models.dart';
import 'api_providers.dart';

part 'message_providers.g.dart';

/// List of messages in a specific channel
@riverpod
Future<List<Message>> channelMessages(
  ChannelMessagesRef ref,
  String channelId,
) async {
  final sessionToken = ref.watch(sessionTokenProvider);
  if (sessionToken == null) {
    throw Exception('Not authenticated');
  }

  final repository = ref.watch(messageRepositoryProvider);
  return await repository.getMessages(
    sessionToken: sessionToken,
    channelId: channelId,
    limit: 50,
  );
}

/// Stream of real-time messages for a channel
@riverpod
Stream<Message> messageStream(
  MessageStreamRef ref,
  String channelId,
) async* {
  final sessionToken = ref.watch(sessionTokenProvider);
  if (sessionToken == null) {
    throw Exception('Not authenticated');
  }

  final repository = ref.watch(messageRepositoryProvider);
  yield* repository.streamMessages(
    sessionToken: sessionToken,
    channelId: channelId,
  );
}

/// Send a message to a channel
@riverpod
class MessageSender extends _$MessageSender {
  @override
  FutureOr<Message?> build() => null;

  Future<Message> sendMessage({
    required String channelId,
    required String content,
    String? replyTo,
  }) async {
    final sessionToken = ref.read(sessionTokenProvider);
    if (sessionToken == null) {
      throw Exception('Not authenticated');
    }

    state = const AsyncValue.loading();

    state = await AsyncValue.guard(() async {
      final repository = ref.read(messageRepositoryProvider);
      return await repository.sendMessage(
        sessionToken: sessionToken,
        channelId: channelId,
        content: content,
        replyTo: replyTo,
      );
    });

    // Invalidate the messages list to refetch
    ref.invalidate(channelMessagesProvider(channelId));

    return state.requireValue!;
  }
}
