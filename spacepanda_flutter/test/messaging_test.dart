import 'package:flutter_test/flutter_test.dart';
import 'package:spacepanda_flutter/api/grpc_client.dart';
import 'package:spacepanda_flutter/generated/spacepanda.pbgrpc.dart';

void main() {
  group('Messaging Tests', () {
    test('Send and receive message in channel', () async {
      final client = SpacePandaGrpcClient();

      try {
        // Setup: Create user, space, and channel
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final username = 'msguser_$timestamp';
        print('Creating user: $username');

        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(username: username, password: 'test123'),
        );
        final token = authResponse.sessionToken;
        print('User created, token: ${token.substring(0, 8)}...');

        // Create space
        print('Creating space...');
        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Message Test Space',
            description: 'For testing messages',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );
        print('Space created: ${spaceResponse.space.id}');

        // Create channel
        print('Creating channel...');
        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'test-chat',
            description: 'Test channel',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
          ),
        );
        print('Channel created: ${channelResponse.channel.id}');

        // Send a message
        print('Sending message...');
        final sentMessage = await client.messages.sendMessage(
          SendMessageRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            content: 'Hello, SpacePanda! 🐼',
          ),
        );

        print('Message sent successfully!');
        print('  ID: ${sentMessage.id}');
        print('  Content: ${sentMessage.content}');
        print('  Channel ID: ${sentMessage.channelId}');
        print('  Sender ID: ${sentMessage.senderId}');

        expect(sentMessage.content, equals('Hello, SpacePanda! 🐼'));
        expect(sentMessage.channelId, equals(channelResponse.channel.id));
        expect(sentMessage.senderId, isNotEmpty);

        // Retrieve messages
        print('Retrieving messages...');
        final messagesResponse = await client.messages.getMessages(
          GetMessagesRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            limit: 50,
          ),
        );

        print('Retrieved ${messagesResponse.messages.length} messages');
        expect(messagesResponse.messages, isNotEmpty);

        // Find our sent message
        final retrievedMessage =
            messagesResponse.messages.firstWhere((m) => m.id == sentMessage.id);
        expect(retrievedMessage.content, equals('Hello, SpacePanda! 🐼'));
        print('SUCCESS: Message send/receive verified!');
      } finally {
        await client.close();
      }
    });

    test('Send multiple messages', () async {
      final client = SpacePandaGrpcClient();

      try {
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: 'multi_$timestamp',
            password: 'test123',
          ),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Multi Message Test',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'busy-channel',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
          ),
        );

        print('Sending 10 messages...');
        final sentMessages = <Message>[];
        for (int i = 1; i <= 10; i++) {
          final message = await client.messages.sendMessage(
            SendMessageRequest(
              sessionToken: token,
              channelId: channelResponse.channel.id,
              content: 'Message number $i',
            ),
          );
          sentMessages.add(message);
          print('  Sent message $i');
        }

        expect(sentMessages.length, equals(10));

        // Retrieve all messages
        final messagesResponse = await client.messages.getMessages(
          GetMessagesRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            limit: 100,
          ),
        );

        expect(messagesResponse.messages.length, greaterThanOrEqualTo(10));

        // Verify all our messages are there
        for (int i = 1; i <= 10; i++) {
          final found = messagesResponse.messages
              .any((m) => m.content == 'Message number $i');
          expect(found, isTrue, reason: 'Should find message number $i');
        }

        print('SUCCESS: All 10 messages sent and retrieved!');
      } finally {
        await client.close();
      }
    });

    test('Message ordering by timestamp', () async {
      final client = SpacePandaGrpcClient();

      try {
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: 'order_$timestamp',
            password: 'test123',
          ),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Order Test',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'ordered-chat',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
          ),
        );

        // Send messages with slight delays
        print('Sending messages with delays...');
        await client.messages.sendMessage(
          SendMessageRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            content: 'First message',
          ),
        );

        await Future.delayed(const Duration(milliseconds: 100));

        await client.messages.sendMessage(
          SendMessageRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            content: 'Second message',
          ),
        );

        await Future.delayed(const Duration(milliseconds: 100));

        await client.messages.sendMessage(
          SendMessageRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            content: 'Third message',
          ),
        );

        // Retrieve messages
        final messagesResponse = await client.messages.getMessages(
          GetMessagesRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            limit: 100,
          ),
        );

        expect(messagesResponse.messages.length, greaterThanOrEqualTo(3));

        // Check timestamps are in ascending order (oldest first)
        final messages = messagesResponse.messages;
        for (int i = 1; i < messages.length; i++) {
          expect(messages[i].timestamp,
              greaterThanOrEqualTo(messages[i - 1].timestamp),
              reason: 'Messages should be in chronological order');
        }

        print('SUCCESS: Messages are properly ordered by timestamp!');
      } finally {
        await client.close();
      }
    });

    test('Empty channel has no messages', () async {
      final client = SpacePandaGrpcClient();

      try {
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: 'empty_$timestamp',
            password: 'test123',
          ),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Empty Test',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'empty-channel',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
          ),
        );

        // Get messages from empty channel
        final messagesResponse = await client.messages.getMessages(
          GetMessagesRequest(
            sessionToken: token,
            channelId: channelResponse.channel.id,
            limit: 50,
          ),
        );

        expect(messagesResponse.messages, isEmpty,
            reason: 'New channel should have no messages');
        print('SUCCESS: Empty channel confirmed!');
      } finally {
        await client.close();
      }
    });
  });
}
