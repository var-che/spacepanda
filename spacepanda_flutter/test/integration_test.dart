import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:spacepanda_flutter/main.dart' as app;
import 'package:spacepanda_flutter/generated/spacepanda.pbgrpc.dart';
import 'package:spacepanda_flutter/api/grpc_client.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  group('SpacePanda Integration Tests', () {
    testWidgets('gRPC Connection Test', (tester) async {
      // Test basic gRPC connection
      final client = SpacePandaGrpcClient();

      try {
        // This should connect successfully
        expect(client.auth, isNotNull);
        expect(client.spaces, isNotNull);
        expect(client.messages, isNotNull);
      } finally {
        await client.close();
      }
    });

    testWidgets('Authentication Flow', (tester) async {
      final client = SpacePandaGrpcClient();

      try {
        // Create a test profile
        final testUsername = 'test_${DateTime.now().millisecondsSinceEpoch}';
        final createResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: testUsername,
            password: 'test123',
          ),
        );

        expect(createResponse.sessionToken, isNotEmpty);
        print(
            '✓ Created profile with token: ${createResponse.sessionToken.substring(0, 8)}...');

        // Lock the profile
        await client.auth.lock(
          LockRequest(sessionToken: createResponse.sessionToken),
        );
        print('✓ Locked profile');

        // Unlock the profile
        final unlockResponse = await client.auth.unlock(
          UnlockRequest(
            username: testUsername,
            password: 'test123',
          ),
        );

        expect(unlockResponse.sessionToken, isNotEmpty);
        print(
            '✓ Unlocked profile with token: ${unlockResponse.sessionToken.substring(0, 8)}...');
      } finally {
        await client.close();
      }
    });

    testWidgets('Spaces API Flow', (tester) async {
      final client = SpacePandaGrpcClient();

      try {
        // Create profile
        final testUsername =
            'spacetest_${DateTime.now().millisecondsSinceEpoch}';
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: testUsername,
            password: 'test123',
          ),
        );

        final token = authResponse.sessionToken;
        print('✓ Created profile for space testing');

        // List spaces (should be empty initially)
        final listResponse = await client.spaces.listSpaces(
          ListSpacesRequest(sessionToken: token),
        );
        expect(listResponse.spaces, isEmpty);
        print('✓ Listed spaces: ${listResponse.spaces.length} spaces');

        // Create a space
        final createSpaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Test Space',
            description: 'A test space',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );
        expect(createSpaceResponse.space.name, equals('Test Space'));
        print('✓ Created space: ${createSpaceResponse.space.name}');

        // List spaces again (should have 1)
        final listResponse2 = await client.spaces.listSpaces(
          ListSpacesRequest(sessionToken: token),
        );
        expect(listResponse2.spaces.length, equals(1));
        print('✓ Listed spaces: ${listResponse2.spaces.length} spaces');

        // List channels (should have default channels)
        final channelsResponse = await client.spaces.listChannels(
          ListChannelsRequest(
            sessionToken: token,
            spaceId: createSpaceResponse.space.id,
          ),
        );
        print(
            '✓ Listed channels: ${channelsResponse.channels.length} channels');
      } finally {
        await client.close();
      }
    });

    testWidgets('Full App Flow', (tester) async {
      // Launch app
      app.main();
      await tester.pumpAndSettle();

      // Should show login screen
      expect(find.text('Create Profile'), findsOneWidget);
      print('✓ App launched - login screen shown');
    });
  });
}
