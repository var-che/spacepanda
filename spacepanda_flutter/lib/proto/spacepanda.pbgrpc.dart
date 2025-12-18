// This is a generated file - do not edit.
//
// Generated from spacepanda.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:async' as $async;
import 'dart:core' as $core;

import 'package:grpc/service_api.dart' as $grpc;
import 'package:protobuf/protobuf.dart' as $pb;

import 'spacepanda.pb.dart' as $0;

export 'spacepanda.pb.dart';

/// Authentication & User Management
@$pb.GrpcServiceName('spacepanda.AuthService')
class AuthServiceClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  AuthServiceClient(super.channel, {super.options, super.interceptors});

  /// Unlock local user profile with password
  $grpc.ResponseFuture<$0.UnlockResponse> unlock(
    $0.UnlockRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$unlock, request, options: options);
  }

  /// Create new local user profile
  $grpc.ResponseFuture<$0.CreateProfileResponse> createProfile(
    $0.CreateProfileRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$createProfile, request, options: options);
  }

  /// Lock current session
  $grpc.ResponseFuture<$0.LockResponse> lock(
    $0.LockRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$lock, request, options: options);
  }

  // method descriptors

  static final _$unlock =
      $grpc.ClientMethod<$0.UnlockRequest, $0.UnlockResponse>(
          '/spacepanda.AuthService/Unlock',
          ($0.UnlockRequest value) => value.writeToBuffer(),
          $0.UnlockResponse.fromBuffer);
  static final _$createProfile =
      $grpc.ClientMethod<$0.CreateProfileRequest, $0.CreateProfileResponse>(
          '/spacepanda.AuthService/CreateProfile',
          ($0.CreateProfileRequest value) => value.writeToBuffer(),
          $0.CreateProfileResponse.fromBuffer);
  static final _$lock = $grpc.ClientMethod<$0.LockRequest, $0.LockResponse>(
      '/spacepanda.AuthService/Lock',
      ($0.LockRequest value) => value.writeToBuffer(),
      $0.LockResponse.fromBuffer);
}

@$pb.GrpcServiceName('spacepanda.AuthService')
abstract class AuthServiceBase extends $grpc.Service {
  $core.String get $name => 'spacepanda.AuthService';

  AuthServiceBase() {
    $addMethod($grpc.ServiceMethod<$0.UnlockRequest, $0.UnlockResponse>(
        'Unlock',
        unlock_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.UnlockRequest.fromBuffer(value),
        ($0.UnlockResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.CreateProfileRequest, $0.CreateProfileResponse>(
            'CreateProfile',
            createProfile_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.CreateProfileRequest.fromBuffer(value),
            ($0.CreateProfileResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.LockRequest, $0.LockResponse>(
        'Lock',
        lock_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.LockRequest.fromBuffer(value),
        ($0.LockResponse value) => value.writeToBuffer()));
  }

  $async.Future<$0.UnlockResponse> unlock_Pre(
      $grpc.ServiceCall $call, $async.Future<$0.UnlockRequest> $request) async {
    return unlock($call, await $request);
  }

  $async.Future<$0.UnlockResponse> unlock(
      $grpc.ServiceCall call, $0.UnlockRequest request);

  $async.Future<$0.CreateProfileResponse> createProfile_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.CreateProfileRequest> $request) async {
    return createProfile($call, await $request);
  }

  $async.Future<$0.CreateProfileResponse> createProfile(
      $grpc.ServiceCall call, $0.CreateProfileRequest request);

  $async.Future<$0.LockResponse> lock_Pre(
      $grpc.ServiceCall $call, $async.Future<$0.LockRequest> $request) async {
    return lock($call, await $request);
  }

  $async.Future<$0.LockResponse> lock(
      $grpc.ServiceCall call, $0.LockRequest request);
}

/// Space Management
@$pb.GrpcServiceName('spacepanda.SpaceService')
class SpaceServiceClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  SpaceServiceClient(super.channel, {super.options, super.interceptors});

  /// List all spaces for current user
  $grpc.ResponseFuture<$0.ListSpacesResponse> listSpaces(
    $0.ListSpacesRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$listSpaces, request, options: options);
  }

  /// Get channels in a space
  $grpc.ResponseFuture<$0.ListChannelsResponse> listChannels(
    $0.ListChannelsRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$listChannels, request, options: options);
  }

  /// Create a new space
  $grpc.ResponseFuture<$0.CreateSpaceResponse> createSpace(
    $0.CreateSpaceRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$createSpace, request, options: options);
  }

  /// Create a new channel in a space
  $grpc.ResponseFuture<$0.CreateChannelResponse> createChannel(
    $0.CreateChannelRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$createChannel, request, options: options);
  }

  /// Get space details
  $grpc.ResponseFuture<$0.Space> getSpace(
    $0.GetSpaceRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getSpace, request, options: options);
  }

  /// Generate a key package for joining channels
  $grpc.ResponseFuture<$0.GenerateKeyPackageResponse> generateKeyPackage(
    $0.GenerateKeyPackageRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$generateKeyPackage, request, options: options);
  }

  /// Create an invite for a user to join a channel (requires their key package)
  $grpc.ResponseFuture<$0.CreateChannelInviteResponse> createChannelInvite(
    $0.CreateChannelInviteRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$createChannelInvite, request, options: options);
  }

  /// Join a channel using an invite token
  $grpc.ResponseFuture<$0.JoinChannelResponse> joinChannel(
    $0.JoinChannelRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$joinChannel, request, options: options);
  }

  /// Add member to channel (legacy - use CreateChannelInvite + JoinChannel instead)
  $grpc.ResponseFuture<$0.AddMemberToChannelResponse> addMemberToChannel(
    $0.AddMemberToChannelRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$addMemberToChannel, request, options: options);
  }

  /// Remove member from channel
  $grpc.ResponseFuture<$0.RemoveMemberFromChannelResponse>
      removeMemberFromChannel(
    $0.RemoveMemberFromChannelRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$removeMemberFromChannel, request,
        options: options);
  }

  // method descriptors

  static final _$listSpaces =
      $grpc.ClientMethod<$0.ListSpacesRequest, $0.ListSpacesResponse>(
          '/spacepanda.SpaceService/ListSpaces',
          ($0.ListSpacesRequest value) => value.writeToBuffer(),
          $0.ListSpacesResponse.fromBuffer);
  static final _$listChannels =
      $grpc.ClientMethod<$0.ListChannelsRequest, $0.ListChannelsResponse>(
          '/spacepanda.SpaceService/ListChannels',
          ($0.ListChannelsRequest value) => value.writeToBuffer(),
          $0.ListChannelsResponse.fromBuffer);
  static final _$createSpace =
      $grpc.ClientMethod<$0.CreateSpaceRequest, $0.CreateSpaceResponse>(
          '/spacepanda.SpaceService/CreateSpace',
          ($0.CreateSpaceRequest value) => value.writeToBuffer(),
          $0.CreateSpaceResponse.fromBuffer);
  static final _$createChannel =
      $grpc.ClientMethod<$0.CreateChannelRequest, $0.CreateChannelResponse>(
          '/spacepanda.SpaceService/CreateChannel',
          ($0.CreateChannelRequest value) => value.writeToBuffer(),
          $0.CreateChannelResponse.fromBuffer);
  static final _$getSpace = $grpc.ClientMethod<$0.GetSpaceRequest, $0.Space>(
      '/spacepanda.SpaceService/GetSpace',
      ($0.GetSpaceRequest value) => value.writeToBuffer(),
      $0.Space.fromBuffer);
  static final _$generateKeyPackage = $grpc.ClientMethod<
          $0.GenerateKeyPackageRequest, $0.GenerateKeyPackageResponse>(
      '/spacepanda.SpaceService/GenerateKeyPackage',
      ($0.GenerateKeyPackageRequest value) => value.writeToBuffer(),
      $0.GenerateKeyPackageResponse.fromBuffer);
  static final _$createChannelInvite = $grpc.ClientMethod<
          $0.CreateChannelInviteRequest, $0.CreateChannelInviteResponse>(
      '/spacepanda.SpaceService/CreateChannelInvite',
      ($0.CreateChannelInviteRequest value) => value.writeToBuffer(),
      $0.CreateChannelInviteResponse.fromBuffer);
  static final _$joinChannel =
      $grpc.ClientMethod<$0.JoinChannelRequest, $0.JoinChannelResponse>(
          '/spacepanda.SpaceService/JoinChannel',
          ($0.JoinChannelRequest value) => value.writeToBuffer(),
          $0.JoinChannelResponse.fromBuffer);
  static final _$addMemberToChannel = $grpc.ClientMethod<
          $0.AddMemberToChannelRequest, $0.AddMemberToChannelResponse>(
      '/spacepanda.SpaceService/AddMemberToChannel',
      ($0.AddMemberToChannelRequest value) => value.writeToBuffer(),
      $0.AddMemberToChannelResponse.fromBuffer);
  static final _$removeMemberFromChannel = $grpc.ClientMethod<
          $0.RemoveMemberFromChannelRequest,
          $0.RemoveMemberFromChannelResponse>(
      '/spacepanda.SpaceService/RemoveMemberFromChannel',
      ($0.RemoveMemberFromChannelRequest value) => value.writeToBuffer(),
      $0.RemoveMemberFromChannelResponse.fromBuffer);
}

@$pb.GrpcServiceName('spacepanda.SpaceService')
abstract class SpaceServiceBase extends $grpc.Service {
  $core.String get $name => 'spacepanda.SpaceService';

  SpaceServiceBase() {
    $addMethod($grpc.ServiceMethod<$0.ListSpacesRequest, $0.ListSpacesResponse>(
        'ListSpaces',
        listSpaces_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.ListSpacesRequest.fromBuffer(value),
        ($0.ListSpacesResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.ListChannelsRequest, $0.ListChannelsResponse>(
            'ListChannels',
            listChannels_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.ListChannelsRequest.fromBuffer(value),
            ($0.ListChannelsResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.CreateSpaceRequest, $0.CreateSpaceResponse>(
            'CreateSpace',
            createSpace_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.CreateSpaceRequest.fromBuffer(value),
            ($0.CreateSpaceResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.CreateChannelRequest, $0.CreateChannelResponse>(
            'CreateChannel',
            createChannel_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.CreateChannelRequest.fromBuffer(value),
            ($0.CreateChannelResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.GetSpaceRequest, $0.Space>(
        'GetSpace',
        getSpace_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.GetSpaceRequest.fromBuffer(value),
        ($0.Space value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.GenerateKeyPackageRequest,
            $0.GenerateKeyPackageResponse>(
        'GenerateKeyPackage',
        generateKeyPackage_Pre,
        false,
        false,
        ($core.List<$core.int> value) =>
            $0.GenerateKeyPackageRequest.fromBuffer(value),
        ($0.GenerateKeyPackageResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.CreateChannelInviteRequest,
            $0.CreateChannelInviteResponse>(
        'CreateChannelInvite',
        createChannelInvite_Pre,
        false,
        false,
        ($core.List<$core.int> value) =>
            $0.CreateChannelInviteRequest.fromBuffer(value),
        ($0.CreateChannelInviteResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.JoinChannelRequest, $0.JoinChannelResponse>(
            'JoinChannel',
            joinChannel_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.JoinChannelRequest.fromBuffer(value),
            ($0.JoinChannelResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.AddMemberToChannelRequest,
            $0.AddMemberToChannelResponse>(
        'AddMemberToChannel',
        addMemberToChannel_Pre,
        false,
        false,
        ($core.List<$core.int> value) =>
            $0.AddMemberToChannelRequest.fromBuffer(value),
        ($0.AddMemberToChannelResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.RemoveMemberFromChannelRequest,
            $0.RemoveMemberFromChannelResponse>(
        'RemoveMemberFromChannel',
        removeMemberFromChannel_Pre,
        false,
        false,
        ($core.List<$core.int> value) =>
            $0.RemoveMemberFromChannelRequest.fromBuffer(value),
        ($0.RemoveMemberFromChannelResponse value) => value.writeToBuffer()));
  }

  $async.Future<$0.ListSpacesResponse> listSpaces_Pre($grpc.ServiceCall $call,
      $async.Future<$0.ListSpacesRequest> $request) async {
    return listSpaces($call, await $request);
  }

  $async.Future<$0.ListSpacesResponse> listSpaces(
      $grpc.ServiceCall call, $0.ListSpacesRequest request);

  $async.Future<$0.ListChannelsResponse> listChannels_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.ListChannelsRequest> $request) async {
    return listChannels($call, await $request);
  }

  $async.Future<$0.ListChannelsResponse> listChannels(
      $grpc.ServiceCall call, $0.ListChannelsRequest request);

  $async.Future<$0.CreateSpaceResponse> createSpace_Pre($grpc.ServiceCall $call,
      $async.Future<$0.CreateSpaceRequest> $request) async {
    return createSpace($call, await $request);
  }

  $async.Future<$0.CreateSpaceResponse> createSpace(
      $grpc.ServiceCall call, $0.CreateSpaceRequest request);

  $async.Future<$0.CreateChannelResponse> createChannel_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.CreateChannelRequest> $request) async {
    return createChannel($call, await $request);
  }

  $async.Future<$0.CreateChannelResponse> createChannel(
      $grpc.ServiceCall call, $0.CreateChannelRequest request);

  $async.Future<$0.Space> getSpace_Pre($grpc.ServiceCall $call,
      $async.Future<$0.GetSpaceRequest> $request) async {
    return getSpace($call, await $request);
  }

  $async.Future<$0.Space> getSpace(
      $grpc.ServiceCall call, $0.GetSpaceRequest request);

  $async.Future<$0.GenerateKeyPackageResponse> generateKeyPackage_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.GenerateKeyPackageRequest> $request) async {
    return generateKeyPackage($call, await $request);
  }

  $async.Future<$0.GenerateKeyPackageResponse> generateKeyPackage(
      $grpc.ServiceCall call, $0.GenerateKeyPackageRequest request);

  $async.Future<$0.CreateChannelInviteResponse> createChannelInvite_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.CreateChannelInviteRequest> $request) async {
    return createChannelInvite($call, await $request);
  }

  $async.Future<$0.CreateChannelInviteResponse> createChannelInvite(
      $grpc.ServiceCall call, $0.CreateChannelInviteRequest request);

  $async.Future<$0.JoinChannelResponse> joinChannel_Pre($grpc.ServiceCall $call,
      $async.Future<$0.JoinChannelRequest> $request) async {
    return joinChannel($call, await $request);
  }

  $async.Future<$0.JoinChannelResponse> joinChannel(
      $grpc.ServiceCall call, $0.JoinChannelRequest request);

  $async.Future<$0.AddMemberToChannelResponse> addMemberToChannel_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.AddMemberToChannelRequest> $request) async {
    return addMemberToChannel($call, await $request);
  }

  $async.Future<$0.AddMemberToChannelResponse> addMemberToChannel(
      $grpc.ServiceCall call, $0.AddMemberToChannelRequest request);

  $async.Future<$0.RemoveMemberFromChannelResponse> removeMemberFromChannel_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.RemoveMemberFromChannelRequest> $request) async {
    return removeMemberFromChannel($call, await $request);
  }

  $async.Future<$0.RemoveMemberFromChannelResponse> removeMemberFromChannel(
      $grpc.ServiceCall call, $0.RemoveMemberFromChannelRequest request);
}

/// Messaging
@$pb.GrpcServiceName('spacepanda.MessageService')
class MessageServiceClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  MessageServiceClient(super.channel, {super.options, super.interceptors});

  /// Get messages for a channel
  $grpc.ResponseFuture<$0.GetMessagesResponse> getMessages(
    $0.GetMessagesRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getMessages, request, options: options);
  }

  /// Send a message
  $grpc.ResponseFuture<$0.Message> sendMessage(
    $0.SendMessageRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$sendMessage, request, options: options);
  }

  /// Stream new messages (real-time)
  $grpc.ResponseStream<$0.Message> streamMessages(
    $0.StreamMessagesRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createStreamingCall(
        _$streamMessages, $async.Stream.fromIterable([request]),
        options: options);
  }

  // method descriptors

  static final _$getMessages =
      $grpc.ClientMethod<$0.GetMessagesRequest, $0.GetMessagesResponse>(
          '/spacepanda.MessageService/GetMessages',
          ($0.GetMessagesRequest value) => value.writeToBuffer(),
          $0.GetMessagesResponse.fromBuffer);
  static final _$sendMessage =
      $grpc.ClientMethod<$0.SendMessageRequest, $0.Message>(
          '/spacepanda.MessageService/SendMessage',
          ($0.SendMessageRequest value) => value.writeToBuffer(),
          $0.Message.fromBuffer);
  static final _$streamMessages =
      $grpc.ClientMethod<$0.StreamMessagesRequest, $0.Message>(
          '/spacepanda.MessageService/StreamMessages',
          ($0.StreamMessagesRequest value) => value.writeToBuffer(),
          $0.Message.fromBuffer);
}

@$pb.GrpcServiceName('spacepanda.MessageService')
abstract class MessageServiceBase extends $grpc.Service {
  $core.String get $name => 'spacepanda.MessageService';

  MessageServiceBase() {
    $addMethod(
        $grpc.ServiceMethod<$0.GetMessagesRequest, $0.GetMessagesResponse>(
            'GetMessages',
            getMessages_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.GetMessagesRequest.fromBuffer(value),
            ($0.GetMessagesResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.SendMessageRequest, $0.Message>(
        'SendMessage',
        sendMessage_Pre,
        false,
        false,
        ($core.List<$core.int> value) =>
            $0.SendMessageRequest.fromBuffer(value),
        ($0.Message value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.StreamMessagesRequest, $0.Message>(
        'StreamMessages',
        streamMessages_Pre,
        false,
        true,
        ($core.List<$core.int> value) =>
            $0.StreamMessagesRequest.fromBuffer(value),
        ($0.Message value) => value.writeToBuffer()));
  }

  $async.Future<$0.GetMessagesResponse> getMessages_Pre($grpc.ServiceCall $call,
      $async.Future<$0.GetMessagesRequest> $request) async {
    return getMessages($call, await $request);
  }

  $async.Future<$0.GetMessagesResponse> getMessages(
      $grpc.ServiceCall call, $0.GetMessagesRequest request);

  $async.Future<$0.Message> sendMessage_Pre($grpc.ServiceCall $call,
      $async.Future<$0.SendMessageRequest> $request) async {
    return sendMessage($call, await $request);
  }

  $async.Future<$0.Message> sendMessage(
      $grpc.ServiceCall call, $0.SendMessageRequest request);

  $async.Stream<$0.Message> streamMessages_Pre($grpc.ServiceCall $call,
      $async.Future<$0.StreamMessagesRequest> $request) async* {
    yield* streamMessages($call, await $request);
  }

  $async.Stream<$0.Message> streamMessages(
      $grpc.ServiceCall call, $0.StreamMessagesRequest request);
}

/// P2P Network
@$pb.GrpcServiceName('spacepanda.NetworkService')
class NetworkServiceClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  NetworkServiceClient(super.channel, {super.options, super.interceptors});

  /// Connect to a peer server
  $grpc.ResponseFuture<$0.ConnectPeerResponse> connectPeer(
    $0.ConnectPeerRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$connectPeer, request, options: options);
  }

  /// Get P2P network status
  $grpc.ResponseFuture<$0.NetworkStatusResponse> getNetworkStatus(
    $0.NetworkStatusRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getNetworkStatus, request, options: options);
  }

  // method descriptors

  static final _$connectPeer =
      $grpc.ClientMethod<$0.ConnectPeerRequest, $0.ConnectPeerResponse>(
          '/spacepanda.NetworkService/ConnectPeer',
          ($0.ConnectPeerRequest value) => value.writeToBuffer(),
          $0.ConnectPeerResponse.fromBuffer);
  static final _$getNetworkStatus =
      $grpc.ClientMethod<$0.NetworkStatusRequest, $0.NetworkStatusResponse>(
          '/spacepanda.NetworkService/GetNetworkStatus',
          ($0.NetworkStatusRequest value) => value.writeToBuffer(),
          $0.NetworkStatusResponse.fromBuffer);
}

@$pb.GrpcServiceName('spacepanda.NetworkService')
abstract class NetworkServiceBase extends $grpc.Service {
  $core.String get $name => 'spacepanda.NetworkService';

  NetworkServiceBase() {
    $addMethod(
        $grpc.ServiceMethod<$0.ConnectPeerRequest, $0.ConnectPeerResponse>(
            'ConnectPeer',
            connectPeer_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.ConnectPeerRequest.fromBuffer(value),
            ($0.ConnectPeerResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.NetworkStatusRequest, $0.NetworkStatusResponse>(
            'GetNetworkStatus',
            getNetworkStatus_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.NetworkStatusRequest.fromBuffer(value),
            ($0.NetworkStatusResponse value) => value.writeToBuffer()));
  }

  $async.Future<$0.ConnectPeerResponse> connectPeer_Pre($grpc.ServiceCall $call,
      $async.Future<$0.ConnectPeerRequest> $request) async {
    return connectPeer($call, await $request);
  }

  $async.Future<$0.ConnectPeerResponse> connectPeer(
      $grpc.ServiceCall call, $0.ConnectPeerRequest request);

  $async.Future<$0.NetworkStatusResponse> getNetworkStatus_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.NetworkStatusRequest> $request) async {
    return getNetworkStatus($call, await $request);
  }

  $async.Future<$0.NetworkStatusResponse> getNetworkStatus(
      $grpc.ServiceCall call, $0.NetworkStatusRequest request);
}
