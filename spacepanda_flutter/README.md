# SpacePanda Flutter

Desktop-first Discord-like UI for SpacePanda secure messaging.

## Features

- 🖥️ **Desktop First** - Optimized for Linux & Windows
- 🔐 **E2EE** - End-to-end encryption via MLS
- 💬 **Spaces & Channels** - Discord-like organization
- 🚀 **gRPC** - High-performance communication with Rust backend
- 🎨 **Modern UI** - Clean, intuitive Discord-inspired interface

## Architecture

```
Flutter App (UI)
    ↓ gRPC
Rust API Server (spacepanda-api)
    ↓
AsyncSpaceManager (Business Logic)
    ↓
MLS Service (Encryption)
```

## Getting Started

### Prerequisites

- Flutter SDK 3.0+
- Rust (for backend)
- Protocol Buffers compiler (`protoc`)

### Installation

1. Install Flutter dependencies:

```bash
cd spacepanda_flutter
flutter pub get
```

2. Run code generation:

```bash
flutter pub run build_runner build --delete-conflicting-outputs
```

3. Run on desktop:

```bash
# Linux
flutter run -d linux

# Windows
flutter run -d windows
```

## Project Structure

```
lib/
├── main.dart                 # App entry point
├── app.dart                  # App widget with routing
│
├── core/                     # Core utilities
│   ├── theme/               # App theme & colors
│   ├── constants/           # Constants & enums
│   └── utils/               # Helper functions
│
├── features/                # Feature modules
│   ├── auth/               # Authentication
│   ├── spaces/             # Spaces list
│   ├── channels/           # Channels & messaging
│   └── settings/           # User settings
│
├── shared/                  # Shared widgets & models
│   ├── models/             # Data models
│   ├── providers/          # Riverpod providers
│   └── widgets/            # Reusable widgets
│
└── api/                     # gRPC & API clients
    ├── grpc/               # Generated gRPC code
    └── mock/               # Mock data for development
```

## Development

### Mock Data Mode

Currently using mock data for UI development. To switch to real backend:

1. Start the Rust gRPC server (TBD)
2. Update API client configuration in `lib/api/config.dart`
3. Rebuild the app

### Code Generation

Run this when you modify models or providers:

```bash
flutter pub run build_runner watch
```

## State Management

Using Riverpod for state management:

- `@riverpod` for provider generation
- `AsyncNotifier` for async state
- `StateNotifier` for complex state

## Planned Features

- [ ] Spaces list sidebar
- [ ] Channels navigation
- [ ] Real-time messaging
- [ ] User profiles
- [ ] Space invites
- [ ] Message search
- [ ] File sharing
- [ ] Voice channels (future)
