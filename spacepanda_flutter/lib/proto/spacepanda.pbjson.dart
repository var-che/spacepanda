// This is a generated file - do not edit.
//
// Generated from spacepanda.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports
// ignore_for_file: unused_import

import 'dart:convert' as $convert;
import 'dart:core' as $core;
import 'dart:typed_data' as $typed_data;

@$core.Deprecated('Use userStatusDescriptor instead')
const UserStatus$json = {
  '1': 'UserStatus',
  '2': [
    {'1': 'USER_STATUS_UNSPECIFIED', '2': 0},
    {'1': 'USER_STATUS_ONLINE', '2': 1},
    {'1': 'USER_STATUS_IDLE', '2': 2},
    {'1': 'USER_STATUS_DND', '2': 3},
    {'1': 'USER_STATUS_OFFLINE', '2': 4},
  ],
};

/// Descriptor for `UserStatus`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List userStatusDescriptor = $convert.base64Decode(
    'CgpVc2VyU3RhdHVzEhsKF1VTRVJfU1RBVFVTX1VOU1BFQ0lGSUVEEAASFgoSVVNFUl9TVEFUVV'
    'NfT05MSU5FEAESFAoQVVNFUl9TVEFUVVNfSURMRRACEhMKD1VTRVJfU1RBVFVTX0RORBADEhcK'
    'E1VTRVJfU1RBVFVTX09GRkxJTkUQBA==');

@$core.Deprecated('Use spaceVisibilityDescriptor instead')
const SpaceVisibility$json = {
  '1': 'SpaceVisibility',
  '2': [
    {'1': 'SPACE_VISIBILITY_UNSPECIFIED', '2': 0},
    {'1': 'SPACE_VISIBILITY_PUBLIC', '2': 1},
    {'1': 'SPACE_VISIBILITY_PRIVATE', '2': 2},
  ],
};

/// Descriptor for `SpaceVisibility`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List spaceVisibilityDescriptor = $convert.base64Decode(
    'Cg9TcGFjZVZpc2liaWxpdHkSIAocU1BBQ0VfVklTSUJJTElUWV9VTlNQRUNJRklFRBAAEhsKF1'
    'NQQUNFX1ZJU0lCSUxJVFlfUFVCTElDEAESHAoYU1BBQ0VfVklTSUJJTElUWV9QUklWQVRFEAI=');

@$core.Deprecated('Use channelVisibilityDescriptor instead')
const ChannelVisibility$json = {
  '1': 'ChannelVisibility',
  '2': [
    {'1': 'CHANNEL_VISIBILITY_UNSPECIFIED', '2': 0},
    {'1': 'CHANNEL_VISIBILITY_PUBLIC', '2': 1},
    {'1': 'CHANNEL_VISIBILITY_PRIVATE', '2': 2},
  ],
};

/// Descriptor for `ChannelVisibility`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List channelVisibilityDescriptor = $convert.base64Decode(
    'ChFDaGFubmVsVmlzaWJpbGl0eRIiCh5DSEFOTkVMX1ZJU0lCSUxJVFlfVU5TUEVDSUZJRUQQAB'
    'IdChlDSEFOTkVMX1ZJU0lCSUxJVFlfUFVCTElDEAESHgoaQ0hBTk5FTF9WSVNJQklMSVRZX1BS'
    'SVZBVEUQAg==');

@$core.Deprecated('Use unlockRequestDescriptor instead')
const UnlockRequest$json = {
  '1': 'UnlockRequest',
  '2': [
    {'1': 'username', '3': 1, '4': 1, '5': 9, '10': 'username'},
    {'1': 'password', '3': 2, '4': 1, '5': 9, '10': 'password'},
  ],
};

/// Descriptor for `UnlockRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List unlockRequestDescriptor = $convert.base64Decode(
    'Cg1VbmxvY2tSZXF1ZXN0EhoKCHVzZXJuYW1lGAEgASgJUgh1c2VybmFtZRIaCghwYXNzd29yZB'
    'gCIAEoCVIIcGFzc3dvcmQ=');

@$core.Deprecated('Use unlockResponseDescriptor instead')
const UnlockResponse$json = {
  '1': 'UnlockResponse',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {
      '1': 'user',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.spacepanda.User',
      '10': 'user'
    },
  ],
};

/// Descriptor for `UnlockResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List unlockResponseDescriptor = $convert.base64Decode(
    'Cg5VbmxvY2tSZXNwb25zZRIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZXNzaW9uVG9rZW4SJA'
    'oEdXNlchgCIAEoCzIQLnNwYWNlcGFuZGEuVXNlclIEdXNlcg==');

@$core.Deprecated('Use createProfileRequestDescriptor instead')
const CreateProfileRequest$json = {
  '1': 'CreateProfileRequest',
  '2': [
    {'1': 'username', '3': 1, '4': 1, '5': 9, '10': 'username'},
    {'1': 'password', '3': 2, '4': 1, '5': 9, '10': 'password'},
  ],
};

/// Descriptor for `CreateProfileRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createProfileRequestDescriptor = $convert.base64Decode(
    'ChRDcmVhdGVQcm9maWxlUmVxdWVzdBIaCgh1c2VybmFtZRgBIAEoCVIIdXNlcm5hbWUSGgoIcG'
    'Fzc3dvcmQYAiABKAlSCHBhc3N3b3Jk');

@$core.Deprecated('Use createProfileResponseDescriptor instead')
const CreateProfileResponse$json = {
  '1': 'CreateProfileResponse',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {
      '1': 'user',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.spacepanda.User',
      '10': 'user'
    },
  ],
};

/// Descriptor for `CreateProfileResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createProfileResponseDescriptor = $convert.base64Decode(
    'ChVDcmVhdGVQcm9maWxlUmVzcG9uc2USIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvbl'
    'Rva2VuEiQKBHVzZXIYAiABKAsyEC5zcGFjZXBhbmRhLlVzZXJSBHVzZXI=');

@$core.Deprecated('Use lockRequestDescriptor instead')
const LockRequest$json = {
  '1': 'LockRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
  ],
};

/// Descriptor for `LockRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List lockRequestDescriptor = $convert.base64Decode(
    'CgtMb2NrUmVxdWVzdBIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZXNzaW9uVG9rZW4=');

@$core.Deprecated('Use lockResponseDescriptor instead')
const LockResponse$json = {
  '1': 'LockResponse',
  '2': [
    {'1': 'success', '3': 1, '4': 1, '5': 8, '10': 'success'},
  ],
};

/// Descriptor for `LockResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List lockResponseDescriptor = $convert
    .base64Decode('CgxMb2NrUmVzcG9uc2USGAoHc3VjY2VzcxgBIAEoCFIHc3VjY2Vzcw==');

@$core.Deprecated('Use listSpacesRequestDescriptor instead')
const ListSpacesRequest$json = {
  '1': 'ListSpacesRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
  ],
};

/// Descriptor for `ListSpacesRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listSpacesRequestDescriptor = $convert.base64Decode(
    'ChFMaXN0U3BhY2VzUmVxdWVzdBIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZXNzaW9uVG9rZW'
    '4=');

@$core.Deprecated('Use listSpacesResponseDescriptor instead')
const ListSpacesResponse$json = {
  '1': 'ListSpacesResponse',
  '2': [
    {
      '1': 'spaces',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.spacepanda.Space',
      '10': 'spaces'
    },
  ],
};

/// Descriptor for `ListSpacesResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listSpacesResponseDescriptor = $convert.base64Decode(
    'ChJMaXN0U3BhY2VzUmVzcG9uc2USKQoGc3BhY2VzGAEgAygLMhEuc3BhY2VwYW5kYS5TcGFjZV'
    'IGc3BhY2Vz');

@$core.Deprecated('Use listChannelsRequestDescriptor instead')
const ListChannelsRequest$json = {
  '1': 'ListChannelsRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'space_id', '3': 2, '4': 1, '5': 9, '10': 'spaceId'},
  ],
};

/// Descriptor for `ListChannelsRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listChannelsRequestDescriptor = $convert.base64Decode(
    'ChNMaXN0Q2hhbm5lbHNSZXF1ZXN0EiMKDXNlc3Npb25fdG9rZW4YASABKAlSDHNlc3Npb25Ub2'
    'tlbhIZCghzcGFjZV9pZBgCIAEoCVIHc3BhY2VJZA==');

@$core.Deprecated('Use listChannelsResponseDescriptor instead')
const ListChannelsResponse$json = {
  '1': 'ListChannelsResponse',
  '2': [
    {
      '1': 'channels',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.spacepanda.Channel',
      '10': 'channels'
    },
  ],
};

/// Descriptor for `ListChannelsResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listChannelsResponseDescriptor = $convert.base64Decode(
    'ChRMaXN0Q2hhbm5lbHNSZXNwb25zZRIvCghjaGFubmVscxgBIAMoCzITLnNwYWNlcGFuZGEuQ2'
    'hhbm5lbFIIY2hhbm5lbHM=');

@$core.Deprecated('Use createSpaceRequestDescriptor instead')
const CreateSpaceRequest$json = {
  '1': 'CreateSpaceRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'name', '3': 2, '4': 1, '5': 9, '10': 'name'},
    {'1': 'description', '3': 3, '4': 1, '5': 9, '10': 'description'},
    {
      '1': 'visibility',
      '3': 4,
      '4': 1,
      '5': 14,
      '6': '.spacepanda.SpaceVisibility',
      '10': 'visibility'
    },
  ],
};

/// Descriptor for `CreateSpaceRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createSpaceRequestDescriptor = $convert.base64Decode(
    'ChJDcmVhdGVTcGFjZVJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2'
    'VuEhIKBG5hbWUYAiABKAlSBG5hbWUSIAoLZGVzY3JpcHRpb24YAyABKAlSC2Rlc2NyaXB0aW9u'
    'EjsKCnZpc2liaWxpdHkYBCABKA4yGy5zcGFjZXBhbmRhLlNwYWNlVmlzaWJpbGl0eVIKdmlzaW'
    'JpbGl0eQ==');

@$core.Deprecated('Use createSpaceResponseDescriptor instead')
const CreateSpaceResponse$json = {
  '1': 'CreateSpaceResponse',
  '2': [
    {
      '1': 'space',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.spacepanda.Space',
      '10': 'space'
    },
  ],
};

/// Descriptor for `CreateSpaceResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createSpaceResponseDescriptor = $convert.base64Decode(
    'ChNDcmVhdGVTcGFjZVJlc3BvbnNlEicKBXNwYWNlGAEgASgLMhEuc3BhY2VwYW5kYS5TcGFjZV'
    'IFc3BhY2U=');

@$core.Deprecated('Use createChannelRequestDescriptor instead')
const CreateChannelRequest$json = {
  '1': 'CreateChannelRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'space_id', '3': 2, '4': 1, '5': 9, '10': 'spaceId'},
    {'1': 'name', '3': 3, '4': 1, '5': 9, '10': 'name'},
    {'1': 'description', '3': 4, '4': 1, '5': 9, '10': 'description'},
    {
      '1': 'visibility',
      '3': 5,
      '4': 1,
      '5': 14,
      '6': '.spacepanda.ChannelVisibility',
      '10': 'visibility'
    },
  ],
};

/// Descriptor for `CreateChannelRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createChannelRequestDescriptor = $convert.base64Decode(
    'ChRDcmVhdGVDaGFubmVsUmVxdWVzdBIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZXNzaW9uVG'
    '9rZW4SGQoIc3BhY2VfaWQYAiABKAlSB3NwYWNlSWQSEgoEbmFtZRgDIAEoCVIEbmFtZRIgCgtk'
    'ZXNjcmlwdGlvbhgEIAEoCVILZGVzY3JpcHRpb24SPQoKdmlzaWJpbGl0eRgFIAEoDjIdLnNwYW'
    'NlcGFuZGEuQ2hhbm5lbFZpc2liaWxpdHlSCnZpc2liaWxpdHk=');

@$core.Deprecated('Use createChannelResponseDescriptor instead')
const CreateChannelResponse$json = {
  '1': 'CreateChannelResponse',
  '2': [
    {
      '1': 'channel',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.spacepanda.Channel',
      '10': 'channel'
    },
  ],
};

/// Descriptor for `CreateChannelResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createChannelResponseDescriptor = $convert.base64Decode(
    'ChVDcmVhdGVDaGFubmVsUmVzcG9uc2USLQoHY2hhbm5lbBgBIAEoCzITLnNwYWNlcGFuZGEuQ2'
    'hhbm5lbFIHY2hhbm5lbA==');

@$core.Deprecated('Use getSpaceRequestDescriptor instead')
const GetSpaceRequest$json = {
  '1': 'GetSpaceRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'space_id', '3': 2, '4': 1, '5': 9, '10': 'spaceId'},
  ],
};

/// Descriptor for `GetSpaceRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List getSpaceRequestDescriptor = $convert.base64Decode(
    'Cg9HZXRTcGFjZVJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2VuEh'
    'kKCHNwYWNlX2lkGAIgASgJUgdzcGFjZUlk');

@$core.Deprecated('Use addMemberToChannelRequestDescriptor instead')
const AddMemberToChannelRequest$json = {
  '1': 'AddMemberToChannelRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'user_id', '3': 3, '4': 1, '5': 9, '10': 'userId'},
  ],
};

/// Descriptor for `AddMemberToChannelRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List addMemberToChannelRequestDescriptor = $convert.base64Decode(
    'ChlBZGRNZW1iZXJUb0NoYW5uZWxSZXF1ZXN0EiMKDXNlc3Npb25fdG9rZW4YASABKAlSDHNlc3'
    'Npb25Ub2tlbhIdCgpjaGFubmVsX2lkGAIgASgJUgljaGFubmVsSWQSFwoHdXNlcl9pZBgDIAEo'
    'CVIGdXNlcklk');

@$core.Deprecated('Use addMemberToChannelResponseDescriptor instead')
const AddMemberToChannelResponse$json = {
  '1': 'AddMemberToChannelResponse',
  '2': [
    {'1': 'success', '3': 1, '4': 1, '5': 8, '10': 'success'},
    {'1': 'message', '3': 2, '4': 1, '5': 9, '10': 'message'},
  ],
};

/// Descriptor for `AddMemberToChannelResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List addMemberToChannelResponseDescriptor =
    $convert.base64Decode(
        'ChpBZGRNZW1iZXJUb0NoYW5uZWxSZXNwb25zZRIYCgdzdWNjZXNzGAEgASgIUgdzdWNjZXNzEh'
        'gKB21lc3NhZ2UYAiABKAlSB21lc3NhZ2U=');

@$core.Deprecated('Use removeMemberFromChannelRequestDescriptor instead')
const RemoveMemberFromChannelRequest$json = {
  '1': 'RemoveMemberFromChannelRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'user_id', '3': 3, '4': 1, '5': 9, '10': 'userId'},
  ],
};

/// Descriptor for `RemoveMemberFromChannelRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List removeMemberFromChannelRequestDescriptor =
    $convert.base64Decode(
        'Ch5SZW1vdmVNZW1iZXJGcm9tQ2hhbm5lbFJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCV'
        'IMc2Vzc2lvblRva2VuEh0KCmNoYW5uZWxfaWQYAiABKAlSCWNoYW5uZWxJZBIXCgd1c2VyX2lk'
        'GAMgASgJUgZ1c2VySWQ=');

@$core.Deprecated('Use removeMemberFromChannelResponseDescriptor instead')
const RemoveMemberFromChannelResponse$json = {
  '1': 'RemoveMemberFromChannelResponse',
  '2': [
    {'1': 'success', '3': 1, '4': 1, '5': 8, '10': 'success'},
    {'1': 'message', '3': 2, '4': 1, '5': 9, '10': 'message'},
  ],
};

/// Descriptor for `RemoveMemberFromChannelResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List removeMemberFromChannelResponseDescriptor =
    $convert.base64Decode(
        'Ch9SZW1vdmVNZW1iZXJGcm9tQ2hhbm5lbFJlc3BvbnNlEhgKB3N1Y2Nlc3MYASABKAhSB3N1Y2'
        'Nlc3MSGAoHbWVzc2FnZRgCIAEoCVIHbWVzc2FnZQ==');

@$core.Deprecated('Use getMessagesRequestDescriptor instead')
const GetMessagesRequest$json = {
  '1': 'GetMessagesRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'limit', '3': 3, '4': 1, '5': 5, '10': 'limit'},
    {'1': 'before', '3': 4, '4': 1, '5': 9, '10': 'before'},
  ],
};

/// Descriptor for `GetMessagesRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List getMessagesRequestDescriptor = $convert.base64Decode(
    'ChJHZXRNZXNzYWdlc1JlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2'
    'VuEh0KCmNoYW5uZWxfaWQYAiABKAlSCWNoYW5uZWxJZBIUCgVsaW1pdBgDIAEoBVIFbGltaXQS'
    'FgoGYmVmb3JlGAQgASgJUgZiZWZvcmU=');

@$core.Deprecated('Use getMessagesResponseDescriptor instead')
const GetMessagesResponse$json = {
  '1': 'GetMessagesResponse',
  '2': [
    {
      '1': 'messages',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.spacepanda.Message',
      '10': 'messages'
    },
  ],
};

/// Descriptor for `GetMessagesResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List getMessagesResponseDescriptor = $convert.base64Decode(
    'ChNHZXRNZXNzYWdlc1Jlc3BvbnNlEi8KCG1lc3NhZ2VzGAEgAygLMhMuc3BhY2VwYW5kYS5NZX'
    'NzYWdlUghtZXNzYWdlcw==');

@$core.Deprecated('Use sendMessageRequestDescriptor instead')
const SendMessageRequest$json = {
  '1': 'SendMessageRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'content', '3': 3, '4': 1, '5': 9, '10': 'content'},
  ],
};

/// Descriptor for `SendMessageRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List sendMessageRequestDescriptor = $convert.base64Decode(
    'ChJTZW5kTWVzc2FnZVJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2'
    'VuEh0KCmNoYW5uZWxfaWQYAiABKAlSCWNoYW5uZWxJZBIYCgdjb250ZW50GAMgASgJUgdjb250'
    'ZW50');

@$core.Deprecated('Use streamMessagesRequestDescriptor instead')
const StreamMessagesRequest$json = {
  '1': 'StreamMessagesRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
  ],
};

/// Descriptor for `StreamMessagesRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List streamMessagesRequestDescriptor = $convert.base64Decode(
    'ChVTdHJlYW1NZXNzYWdlc1JlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvbl'
    'Rva2VuEh0KCmNoYW5uZWxfaWQYAiABKAlSCWNoYW5uZWxJZA==');

@$core.Deprecated('Use userDescriptor instead')
const User$json = {
  '1': 'User',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 9, '10': 'id'},
    {'1': 'username', '3': 2, '4': 1, '5': 9, '10': 'username'},
    {'1': 'display_name', '3': 3, '4': 1, '5': 9, '10': 'displayName'},
    {'1': 'avatar_url', '3': 4, '4': 1, '5': 9, '10': 'avatarUrl'},
    {
      '1': 'status',
      '3': 5,
      '4': 1,
      '5': 14,
      '6': '.spacepanda.UserStatus',
      '10': 'status'
    },
  ],
};

/// Descriptor for `User`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List userDescriptor = $convert.base64Decode(
    'CgRVc2VyEg4KAmlkGAEgASgJUgJpZBIaCgh1c2VybmFtZRgCIAEoCVIIdXNlcm5hbWUSIQoMZG'
    'lzcGxheV9uYW1lGAMgASgJUgtkaXNwbGF5TmFtZRIdCgphdmF0YXJfdXJsGAQgASgJUglhdmF0'
    'YXJVcmwSLgoGc3RhdHVzGAUgASgOMhYuc3BhY2VwYW5kYS5Vc2VyU3RhdHVzUgZzdGF0dXM=');

@$core.Deprecated('Use spaceDescriptor instead')
const Space$json = {
  '1': 'Space',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 9, '10': 'id'},
    {'1': 'name', '3': 2, '4': 1, '5': 9, '10': 'name'},
    {'1': 'description', '3': 3, '4': 1, '5': 9, '10': 'description'},
    {'1': 'icon_url', '3': 4, '4': 1, '5': 9, '10': 'iconUrl'},
    {
      '1': 'visibility',
      '3': 5,
      '4': 1,
      '5': 14,
      '6': '.spacepanda.SpaceVisibility',
      '10': 'visibility'
    },
    {'1': 'owner_id', '3': 6, '4': 1, '5': 9, '10': 'ownerId'},
    {'1': 'member_ids', '3': 7, '4': 3, '5': 9, '10': 'memberIds'},
    {'1': 'channel_ids', '3': 8, '4': 3, '5': 9, '10': 'channelIds'},
    {'1': 'created_at', '3': 9, '4': 1, '5': 3, '10': 'createdAt'},
  ],
};

/// Descriptor for `Space`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List spaceDescriptor = $convert.base64Decode(
    'CgVTcGFjZRIOCgJpZBgBIAEoCVICaWQSEgoEbmFtZRgCIAEoCVIEbmFtZRIgCgtkZXNjcmlwdG'
    'lvbhgDIAEoCVILZGVzY3JpcHRpb24SGQoIaWNvbl91cmwYBCABKAlSB2ljb25VcmwSOwoKdmlz'
    'aWJpbGl0eRgFIAEoDjIbLnNwYWNlcGFuZGEuU3BhY2VWaXNpYmlsaXR5Ugp2aXNpYmlsaXR5Eh'
    'kKCG93bmVyX2lkGAYgASgJUgdvd25lcklkEh0KCm1lbWJlcl9pZHMYByADKAlSCW1lbWJlcklk'
    'cxIfCgtjaGFubmVsX2lkcxgIIAMoCVIKY2hhbm5lbElkcxIdCgpjcmVhdGVkX2F0GAkgASgDUg'
    'ljcmVhdGVkQXQ=');

@$core.Deprecated('Use channelDescriptor instead')
const Channel$json = {
  '1': 'Channel',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 9, '10': 'id'},
    {'1': 'space_id', '3': 2, '4': 1, '5': 9, '10': 'spaceId'},
    {'1': 'name', '3': 3, '4': 1, '5': 9, '10': 'name'},
    {'1': 'description', '3': 4, '4': 1, '5': 9, '10': 'description'},
    {
      '1': 'visibility',
      '3': 5,
      '4': 1,
      '5': 14,
      '6': '.spacepanda.ChannelVisibility',
      '10': 'visibility'
    },
    {'1': 'member_ids', '3': 6, '4': 3, '5': 9, '10': 'memberIds'},
    {'1': 'created_at', '3': 7, '4': 1, '5': 3, '10': 'createdAt'},
  ],
};

/// Descriptor for `Channel`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List channelDescriptor = $convert.base64Decode(
    'CgdDaGFubmVsEg4KAmlkGAEgASgJUgJpZBIZCghzcGFjZV9pZBgCIAEoCVIHc3BhY2VJZBISCg'
    'RuYW1lGAMgASgJUgRuYW1lEiAKC2Rlc2NyaXB0aW9uGAQgASgJUgtkZXNjcmlwdGlvbhI9Cgp2'
    'aXNpYmlsaXR5GAUgASgOMh0uc3BhY2VwYW5kYS5DaGFubmVsVmlzaWJpbGl0eVIKdmlzaWJpbG'
    'l0eRIdCgptZW1iZXJfaWRzGAYgAygJUgltZW1iZXJJZHMSHQoKY3JlYXRlZF9hdBgHIAEoA1IJ'
    'Y3JlYXRlZEF0');

@$core.Deprecated('Use messageDescriptor instead')
const Message$json = {
  '1': 'Message',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 9, '10': 'id'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'sender_id', '3': 3, '4': 1, '5': 9, '10': 'senderId'},
    {'1': 'content', '3': 4, '4': 1, '5': 9, '10': 'content'},
    {'1': 'timestamp', '3': 5, '4': 1, '5': 3, '10': 'timestamp'},
    {'1': 'is_e2ee', '3': 6, '4': 1, '5': 8, '10': 'isE2ee'},
    {'1': 'attachments', '3': 7, '4': 3, '5': 9, '10': 'attachments'},
  ],
};

/// Descriptor for `Message`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List messageDescriptor = $convert.base64Decode(
    'CgdNZXNzYWdlEg4KAmlkGAEgASgJUgJpZBIdCgpjaGFubmVsX2lkGAIgASgJUgljaGFubmVsSW'
    'QSGwoJc2VuZGVyX2lkGAMgASgJUghzZW5kZXJJZBIYCgdjb250ZW50GAQgASgJUgdjb250ZW50'
    'EhwKCXRpbWVzdGFtcBgFIAEoA1IJdGltZXN0YW1wEhcKB2lzX2UyZWUYBiABKAhSBmlzRTJlZR'
    'IgCgthdHRhY2htZW50cxgHIAMoCVILYXR0YWNobWVudHM=');

@$core.Deprecated('Use generateKeyPackageRequestDescriptor instead')
const GenerateKeyPackageRequest$json = {
  '1': 'GenerateKeyPackageRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
  ],
};

/// Descriptor for `GenerateKeyPackageRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List generateKeyPackageRequestDescriptor =
    $convert.base64Decode(
        'ChlHZW5lcmF0ZUtleVBhY2thZ2VSZXF1ZXN0EiMKDXNlc3Npb25fdG9rZW4YASABKAlSDHNlc3'
        'Npb25Ub2tlbg==');

@$core.Deprecated('Use generateKeyPackageResponseDescriptor instead')
const GenerateKeyPackageResponse$json = {
  '1': 'GenerateKeyPackageResponse',
  '2': [
    {'1': 'key_package', '3': 1, '4': 1, '5': 12, '10': 'keyPackage'},
  ],
};

/// Descriptor for `GenerateKeyPackageResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List generateKeyPackageResponseDescriptor =
    $convert.base64Decode(
        'ChpHZW5lcmF0ZUtleVBhY2thZ2VSZXNwb25zZRIfCgtrZXlfcGFja2FnZRgBIAEoDFIKa2V5UG'
        'Fja2FnZQ==');

@$core.Deprecated('Use createChannelInviteRequestDescriptor instead')
const CreateChannelInviteRequest$json = {
  '1': 'CreateChannelInviteRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'key_package', '3': 3, '4': 1, '5': 12, '10': 'keyPackage'},
  ],
};

/// Descriptor for `CreateChannelInviteRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createChannelInviteRequestDescriptor =
    $convert.base64Decode(
        'ChpDcmVhdGVDaGFubmVsSW52aXRlUmVxdWVzdBIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZX'
        'NzaW9uVG9rZW4SHQoKY2hhbm5lbF9pZBgCIAEoCVIJY2hhbm5lbElkEh8KC2tleV9wYWNrYWdl'
        'GAMgASgMUgprZXlQYWNrYWdl');

@$core.Deprecated('Use createChannelInviteResponseDescriptor instead')
const CreateChannelInviteResponse$json = {
  '1': 'CreateChannelInviteResponse',
  '2': [
    {'1': 'invite_token', '3': 1, '4': 1, '5': 12, '10': 'inviteToken'},
    {'1': 'commit', '3': 2, '4': 1, '5': 12, '10': 'commit'},
    {'1': 'ratchet_tree', '3': 3, '4': 1, '5': 12, '10': 'ratchetTree'},
    {'1': 'space_id', '3': 4, '4': 1, '5': 9, '10': 'spaceId'},
    {'1': 'channel_name', '3': 5, '4': 1, '5': 9, '10': 'channelName'},
  ],
};

/// Descriptor for `CreateChannelInviteResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List createChannelInviteResponseDescriptor = $convert.base64Decode(
    'ChtDcmVhdGVDaGFubmVsSW52aXRlUmVzcG9uc2USIQoMaW52aXRlX3Rva2VuGAEgASgMUgtpbn'
    'ZpdGVUb2tlbhIWCgZjb21taXQYAiABKAxSBmNvbW1pdBIhCgxyYXRjaGV0X3RyZWUYAyABKAxS'
    'C3JhdGNoZXRUcmVlEhkKCHNwYWNlX2lkGAQgASgJUgdzcGFjZUlkEiEKDGNoYW5uZWxfbmFtZR'
    'gFIAEoCVILY2hhbm5lbE5hbWU=');

@$core.Deprecated('Use joinChannelRequestDescriptor instead')
const JoinChannelRequest$json = {
  '1': 'JoinChannelRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'invite_token', '3': 2, '4': 1, '5': 12, '10': 'inviteToken'},
    {'1': 'ratchet_tree', '3': 3, '4': 1, '5': 12, '10': 'ratchetTree'},
    {'1': 'space_id', '3': 4, '4': 1, '5': 9, '10': 'spaceId'},
    {'1': 'channel_name', '3': 5, '4': 1, '5': 9, '10': 'channelName'},
  ],
};

/// Descriptor for `JoinChannelRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List joinChannelRequestDescriptor = $convert.base64Decode(
    'ChJKb2luQ2hhbm5lbFJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2'
    'VuEiEKDGludml0ZV90b2tlbhgCIAEoDFILaW52aXRlVG9rZW4SIQoMcmF0Y2hldF90cmVlGAMg'
    'ASgMUgtyYXRjaGV0VHJlZRIZCghzcGFjZV9pZBgEIAEoCVIHc3BhY2VJZBIhCgxjaGFubmVsX2'
    '5hbWUYBSABKAlSC2NoYW5uZWxOYW1l');

@$core.Deprecated('Use joinChannelResponseDescriptor instead')
const JoinChannelResponse$json = {
  '1': 'JoinChannelResponse',
  '2': [
    {'1': 'success', '3': 1, '4': 1, '5': 8, '10': 'success'},
    {'1': 'channel_id', '3': 2, '4': 1, '5': 9, '10': 'channelId'},
    {'1': 'message', '3': 3, '4': 1, '5': 9, '10': 'message'},
  ],
};

/// Descriptor for `JoinChannelResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List joinChannelResponseDescriptor = $convert.base64Decode(
    'ChNKb2luQ2hhbm5lbFJlc3BvbnNlEhgKB3N1Y2Nlc3MYASABKAhSB3N1Y2Nlc3MSHQoKY2hhbm'
    '5lbF9pZBgCIAEoCVIJY2hhbm5lbElkEhgKB21lc3NhZ2UYAyABKAlSB21lc3NhZ2U=');

@$core.Deprecated('Use connectPeerRequestDescriptor instead')
const ConnectPeerRequest$json = {
  '1': 'ConnectPeerRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
    {'1': 'peer_address', '3': 2, '4': 1, '5': 9, '10': 'peerAddress'},
  ],
};

/// Descriptor for `ConnectPeerRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List connectPeerRequestDescriptor = $convert.base64Decode(
    'ChJDb25uZWN0UGVlclJlcXVlc3QSIwoNc2Vzc2lvbl90b2tlbhgBIAEoCVIMc2Vzc2lvblRva2'
    'VuEiEKDHBlZXJfYWRkcmVzcxgCIAEoCVILcGVlckFkZHJlc3M=');

@$core.Deprecated('Use connectPeerResponseDescriptor instead')
const ConnectPeerResponse$json = {
  '1': 'ConnectPeerResponse',
  '2': [
    {'1': 'success', '3': 1, '4': 1, '5': 8, '10': 'success'},
    {'1': 'message', '3': 2, '4': 1, '5': 9, '10': 'message'},
  ],
};

/// Descriptor for `ConnectPeerResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List connectPeerResponseDescriptor = $convert.base64Decode(
    'ChNDb25uZWN0UGVlclJlc3BvbnNlEhgKB3N1Y2Nlc3MYASABKAhSB3N1Y2Nlc3MSGAoHbWVzc2'
    'FnZRgCIAEoCVIHbWVzc2FnZQ==');

@$core.Deprecated('Use networkStatusRequestDescriptor instead')
const NetworkStatusRequest$json = {
  '1': 'NetworkStatusRequest',
  '2': [
    {'1': 'session_token', '3': 1, '4': 1, '5': 9, '10': 'sessionToken'},
  ],
};

/// Descriptor for `NetworkStatusRequest`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List networkStatusRequestDescriptor = $convert.base64Decode(
    'ChROZXR3b3JrU3RhdHVzUmVxdWVzdBIjCg1zZXNzaW9uX3Rva2VuGAEgASgJUgxzZXNzaW9uVG'
    '9rZW4=');

@$core.Deprecated('Use networkStatusResponseDescriptor instead')
const NetworkStatusResponse$json = {
  '1': 'NetworkStatusResponse',
  '2': [
    {'1': 'peer_id', '3': 1, '4': 1, '5': 9, '10': 'peerId'},
    {'1': 'listen_address', '3': 2, '4': 1, '5': 9, '10': 'listenAddress'},
    {'1': 'connected_peers', '3': 3, '4': 3, '5': 9, '10': 'connectedPeers'},
  ],
};

/// Descriptor for `NetworkStatusResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List networkStatusResponseDescriptor = $convert.base64Decode(
    'ChVOZXR3b3JrU3RhdHVzUmVzcG9uc2USFwoHcGVlcl9pZBgBIAEoCVIGcGVlcklkEiUKDmxpc3'
    'Rlbl9hZGRyZXNzGAIgASgJUg1saXN0ZW5BZGRyZXNzEicKD2Nvbm5lY3RlZF9wZWVycxgDIAMo'
    'CVIOY29ubmVjdGVkUGVlcnM=');
