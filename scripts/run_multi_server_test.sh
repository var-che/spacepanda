#!/bin/bash
# Multi-server test runner for P2P testing
#
# Usage: ./scripts/run_multi_server_test.sh

set -e

echo "🐼 Starting Multi-Server P2P Test"
echo "=================================="

# Kill any existing spacepanda-api processes
pkill -f spacepanda-api || true
sleep 1

# Start 3 API servers on different ports
echo "Starting API servers..."

# Server 1 - Port 50051
RUST_LOG=info nix develop --command cargo run --bin spacepanda-api -- --port 50051 > /tmp/spacepanda_50051.log 2>&1 &
SERVER1_PID=$!
echo "Server 1: PID $SERVER1_PID (port 50051)"

# Server 2 - Port 50052
RUST_LOG=info nix develop --command cargo run --bin spacepanda-api -- --port 50052 > /tmp/spacepanda_50052.log 2>&1 &
SERVER2_PID=$!
echo "Server 2: PID $SERVER2_PID (port 50052)"

# Server 3 - Port 50053
RUST_LOG=info nix develop --command cargo run --bin spacepanda-api -- --port 50053 > /tmp/spacepanda_50053.log 2>&1 &
SERVER3_PID=$!
echo "Server 3: PID $SERVER3_PID (port 50053)"

# Wait for servers to start
echo "Waiting for servers to start..."
sleep 3

# Function to cleanup on exit
cleanup() {
    echo "Shutting down servers..."
    kill $SERVER1_PID $SERVER2_PID $SERVER3_PID 2>/dev/null || true
    echo "Servers stopped"
}

trap cleanup EXIT

# Run Flutter test
echo ""
echo "Running multi-server P2P tests..."
cd spacepanda_flutter
flutter test test/p2p_multi_server_test.dart

echo ""
echo "✅ Tests complete!"
echo ""
echo "Server logs:"
echo "  Server 1: /tmp/spacepanda_50051.log"
echo "  Server 2: /tmp/spacepanda_50052.log"
echo "  Server 3: /tmp/spacepanda_50053.log"
