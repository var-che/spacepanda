import 'package:grpc/grpc.dart';
import '../generated/spacepanda.pbgrpc.dart';

/// Global gRPC client configuration
class SpacePandaGrpcClient {
  static const String defaultHost = '127.0.0.1';
  static const int defaultPort = 50051;

  late final ClientChannel _channel;
  late final AuthServiceClient _authService;
  late final SpaceServiceClient _spaceService;
  late final MessageServiceClient _messageService;

  SpacePandaGrpcClient({
    String host = defaultHost,
    int port = defaultPort,
  }) {
    print('SpacePandaGrpcClient: Initializing connection to $host:$port');
    _channel = ClientChannel(
      host,
      port: port,
      options: const ChannelOptions(
        credentials: ChannelCredentials.insecure(),
        connectionTimeout: Duration(seconds: 10),
        idleTimeout: Duration(minutes: 5),
        keepAlive: ClientKeepAliveOptions(
          pingInterval: Duration(seconds: 30),
          timeout: Duration(seconds: 10),
          permitWithoutCalls: true,
        ),
      ),
    );

    _authService = AuthServiceClient(_channel);
    _spaceService = SpaceServiceClient(_channel);
    _messageService = MessageServiceClient(_channel);
    print('SpacePandaGrpcClient: Client initialized successfully');
  }

  AuthServiceClient get auth => _authService;
  SpaceServiceClient get spaces => _spaceService;
  MessageServiceClient get messages => _messageService;

  Future<void> close() async {
    await _channel.shutdown();
  }
}
