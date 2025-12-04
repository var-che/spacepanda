#!/usr/bin/env bash
# Test script for member removal HTTP endpoint
# Prerequisites: Start the HTTP test harness server first

set -e

BASE_URL="http://localhost:3000"

echo "=== Testing Member Removal Flow ==="

# Create 3 identities
echo -n "Creating Alice... "
ALICE=$(curl -s -X POST "$BASE_URL/identity/create" | jq -r '.user_id')
echo "✓ $ALICE"

echo -n "Creating Bob... "
BOB=$(curl -s -X POST "$BASE_URL/identity/create" | jq -r '.user_id')
echo "✓ $BOB"

echo -n "Creating Charlie... "
CHARLIE=$(curl -s -X POST "$BASE_URL/identity/create" | jq -r '.user_id')
echo "✓ $CHARLIE"

# Alice creates a channel
echo -n "Alice creates channel... "
CHANNEL=$(curl -s -X POST "$BASE_URL/channels/create" \
  -H "Content-Type: application/json" \
  -d '{"name": "test-removal", "is_public": true}' | jq -r '.channel_id')
echo "✓ $CHANNEL"

# Alice invites Bob
echo -n "Alice invites Bob... "
BOB_INVITE=$(curl -s -X POST "$BASE_URL/channels/$CHANNEL/invite" \
  -H "Content-Type: application/json" \
  -d "{\"invitee_user_id\": \"$BOB\"}" | jq -r '.commit')
echo "✓"

# Bob processes invite and joins
echo -n "Bob joins... "
curl -s -X POST "$BASE_URL/channels/$CHANNEL/process-commit" \
  -H "Content-Type: application/json" \
  -d "{\"commit\": $BOB_INVITE}" > /dev/null
echo "✓"

# Alice invites Charlie
echo -n "Alice invites Charlie... "
CHARLIE_INVITE=$(curl -s -X POST "$BASE_URL/channels/$CHANNEL/invite" \
  -H "Content-Type: application/json" \
  -d "{\"invitee_user_id\": \"$CHARLIE\"}" | jq -r '.commit')
echo "✓"

# Bob and Charlie process commits
echo -n "Processing commits... "
curl -s -X POST "$BASE_URL/channels/$CHANNEL/process-commit" \
  -H "Content-Type: application/json" \
  -d "{\"commit\": $CHARLIE_INVITE}" > /dev/null
echo "✓"

# List members
echo "Members before removal:"
curl -s "$BASE_URL/channels/$CHANNEL/members" | jq '.members'

# Alice removes Bob
echo -n "Alice removes Bob... "
REMOVAL_COMMIT=$(curl -s -X POST "$BASE_URL/channels/$CHANNEL/remove-member" \
  -H "Content-Type: application/json" \
  -d "{\"member_id\": \"$BOB\"}" | jq -r '.commit')
echo "✓"

# Charlie processes the removal commit
echo -n "Charlie processes removal... "
curl -s -X POST "$BASE_URL/channels/$CHANNEL/process-commit" \
  -H "Content-Type: application/json" \
  -d "{\"commit\": $REMOVAL_COMMIT}" > /dev/null
echo "✓"

# List members after removal
echo "Members after removal:"
curl -s "$BASE_URL/channels/$CHANNEL/members" | jq '.members'

echo ""
echo "=== Member Removal Test Complete ==="
echo "Expected: Bob should no longer be in the member list"
