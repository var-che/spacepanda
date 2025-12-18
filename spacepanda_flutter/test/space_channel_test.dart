import 'package:flutter_test/flutter_test.dart';
import 'package:spacepanda_flutter/generated/spacepanda.pbgrpc.dart';
import 'package:spacepanda_flutter/api/grpc_client.dart';

/// Comprehensive tests for Space and Channel creation
void main() {
  group('Space Management Integration Tests', () {
    late SpacePandaGrpcClient client;

    setUp(() {
      client = SpacePandaGrpcClient();
    });

    tearDown(() async {
      await client.close();
    });

    test('Create space and verify it appears in list', () async {
      // Create auth
      final testUsername = 'spacetest_${DateTime.now().millisecondsSinceEpoch}';
      final authResponse = await client.auth.createProfile(
        CreateProfileRequest(
          username: testUsername,
          password: 'test123',
        ),
      );

      final token = authResponse.sessionToken;
      expect(token, isNotEmpty);
      print('Created profile with token: ${token.substring(0, 8)}...');

      // List spaces (should be empty)
      final initialSpaces = await client.spaces.listSpaces(
        ListSpacesRequest(sessionToken: token),
      );
      final initialCount = initialSpaces.spaces.length;
      print('Initial spaces count: $initialCount');

      // Create first space
      final space1 = await client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: token,
          name: 'Gaming Hub',
          description: 'For all gaming discussions',
          visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
        ),
      );
      expect(space1.space.name, equals('Gaming Hub'));
      expect(space1.space.description, equals('For all gaming discussions'));
      print('Created space: ${space1.space.name} (${space1.space.id})');

      // Create second space
      final space2 = await client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: token,
          name: 'Work Projects',
          description: 'Professional workspace',
          visibility: SpaceVisibility.SPACE_VISIBILITY_PRIVATE,
        ),
      );
      expect(space2.space.name, equals('Work Projects'));
      print('Created space: ${space2.space.name} (${space2.space.id})');

      // List spaces again
      final finalSpaces = await client.spaces.listSpaces(
        ListSpacesRequest(sessionToken: token),
      );
      expect(finalSpaces.spaces.length, equals(initialCount + 2));
      print('Final spaces count: ${finalSpaces.spaces.length}');

      // Verify both spaces are in the list
      final spaceNames = finalSpaces.spaces.map((s) => s.name).toList();
      expect(spaceNames, contains('Gaming Hub'));
      expect(spaceNames, contains('Work Projects'));
      print('All spaces verified in list');
    });

    test('Create space with empty name should fail gracefully', () async {
      final testUsername = 'failtest_${DateTime.now().millisecondsSinceEpoch}';
      final authResponse = await client.auth.createProfile(
        CreateProfileRequest(
          username: testUsername,
          password: 'test123',
        ),
      );

      final token = authResponse.sessionToken;

      // Try to create space with empty name
      try {
        await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: '',
            description: 'Test',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );
        fail('Should have thrown an error for empty space name');
      } catch (e) {
        print('Correctly rejected empty space name: $e');
        expect(e.toString(), contains('INVALID_ARGUMENT'));
      }
    });

    test('List channels in newly created space', () async {
      final testUsername = 'chantest_${DateTime.now().millisecondsSinceEpoch}';
      final authResponse = await client.auth.createProfile(
        CreateProfileRequest(
          username: testUsername,
          password: 'test123',
        ),
      );

      final token = authResponse.sessionToken;

      // Create a space
      final spaceResponse = await client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: token,
          name: 'Channel Test Space',
          description: 'Testing channels',
          visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
        ),
      );
      print('Created space: ${spaceResponse.space.name}');

      // List channels (backend should create default channels)
      final channelsResponse = await client.spaces.listChannels(
        ListChannelsRequest(
          sessionToken: token,
          spaceId: spaceResponse.space.id,
        ),
      );
      print('Found ${channelsResponse.channels.length} channels');

      // Verify we have some channels
      expect(channelsResponse.channels, isNotEmpty,
          reason: 'New space should have at least one default channel');

      for (final channel in channelsResponse.channels) {
        print('  - Channel: ${channel.name} (${channel.id})');
        expect(channel.spaceId, equals(spaceResponse.space.id));
        expect(channel.name, isNotEmpty);
      }
    });

    test('Multiple users should have isolated spaces', () async {
      // Create first user
      final user1 = 'user1_${DateTime.now().millisecondsSinceEpoch}';
      final auth1 = await client.auth.createProfile(
        CreateProfileRequest(username: user1, password: 'pass1'),
      );
      print('Created user1: $user1');

      // Create space for user1
      await client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: auth1.sessionToken,
          name: 'User1 Private Space',
          description: 'Only for user1',
          visibility: SpaceVisibility.SPACE_VISIBILITY_PRIVATE,
        ),
      );

      // Create second user
      final user2 = 'user2_${DateTime.now().millisecondsSinceEpoch}';
      final auth2 = await client.auth.createProfile(
        CreateProfileRequest(username: user2, password: 'pass2'),
      );
      print('Created user2: $user2');

      // List spaces for user2 (should not see user1's space)
      final user2Spaces = await client.spaces.listSpaces(
        ListSpacesRequest(sessionToken: auth2.sessionToken),
      );

      expect(user2Spaces.spaces, isEmpty,
          reason: 'User2 should not see User1\'s private space');
      print('User isolation verified');
    });
  });

  group('Channel Management Tests', () {
    late SpacePandaGrpcClient client;
    late String token;
    late String spaceId;

    setUp(() async {
      client = SpacePandaGrpcClient();

      // Create user and space for channel tests
      final testUsername = 'chantest_${DateTime.now().millisecondsSinceEpoch}';
      final authResponse = await client.auth.createProfile(
        CreateProfileRequest(username: testUsername, password: 'test123'),
      );
      token = authResponse.sessionToken;

      final spaceResponse = await client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: token,
          name: 'Test Space',
          description: 'For testing channels',
          visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
        ),
      );
      spaceId = spaceResponse.space.id;
    });

    tearDown(() async {
      await client.close();
    });

    test('Channels have correct space_id relationship', () async {
      final channelsResponse = await client.spaces.listChannels(
        ListChannelsRequest(sessionToken: token, spaceId: spaceId),
      );

      for (final channel in channelsResponse.channels) {
        expect(channel.spaceId, equals(spaceId),
            reason: 'All channels should belong to the space');
        print('Channel ${channel.name} belongs to space $spaceId');
      }
    });

    test('Channels have valid timestamps', () async {
      final channelsResponse = await client.spaces.listChannels(
        ListChannelsRequest(sessionToken: token, spaceId: spaceId),
      );

      final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      for (final channel in channelsResponse.channels) {
        expect(channel.createdAt, greaterThan(0));
        expect(channel.createdAt, lessThanOrEqualTo(now + 10),
            reason: 'Timestamp should be recent');
        print(
            'Channel ${channel.name} has valid timestamp: ${channel.createdAt}');
      }
    });
  });

  group('Message Flow Tests', () {
    test('Send and retrieve message in channel', () async {
      final client = SpacePandaGrpcClient();

      try {
        // Setup: create user, space, get channel
        final testUsername = 'msgtest_${DateTime.now().millisecondsSinceEpoch}';
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(username: testUsername, password: 'test123'),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Message Test Space',
            description: 'For testing messages',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        final channelsResponse = await client.spaces.listChannels(
          ListChannelsRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
          ),
        );

        expect(channelsResponse.channels, isNotEmpty);
        final channelId = channelsResponse.channels.first.id;
        print('Setup complete, using channel: $channelId');

        // Send message
        final sentMessage = await client.messages.sendMessage(
          SendMessageRequest(
            sessionToken: token,
            channelId: channelId,
            content: 'Hello, SpacePanda! 🐼',
          ),
        );

        expect(sentMessage.content, equals('Hello, SpacePanda! 🐼'));
        expect(sentMessage.channelId, equals(channelId));
        expect(sentMessage.senderId, isNotEmpty);
        print('Sent message: ${sentMessage.content}');

        // Retrieve messages
        final messagesResponse = await client.messages.getMessages(
          GetMessagesRequest(
            sessionToken: token,
            channelId: channelId,
            limit: 50,
          ),
        );

        expect(messagesResponse.messages, isNotEmpty);
        final retrievedMessage =
            messagesResponse.messages.firstWhere((m) => m.id == sentMessage.id);
        expect(retrievedMessage.content, equals('Hello, SpacePanda! 🐼'));
        print('Retrieved message successfully');
      } finally {
        await client.close();
      }
    });
  });
}
