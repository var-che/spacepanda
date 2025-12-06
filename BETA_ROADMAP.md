# SpacePanda Beta Roadmap

**Version**: 1.0  
**Date**: December 6, 2025  
**Target Beta Release**: February 2026 (8-10 weeks)  
**Current Status**: Alpha/MVP

---

## Executive Summary

SpacePanda has a production-ready MLS core (1205+ tests passing, A+ cryptography) but needs critical infrastructure work to reach beta. This roadmap focuses on the three blocking items: **Network Layer → Persistence → Security Audit**.

**Current State**:

- ✅ MLS Protocol: Production-ready
- ✅ Privacy Features: 90% complete (mixing/scoping ready)
- ⚠️ Network Layer: 60% complete (TCP works, sessions don't)
- 🔴 Persistence: 20% complete (snapshots only, no recovery)
- 🔴 Security Audit: Not started

---

## Phase 1: Network Layer Completion (Weeks 1-4)

**Goal**: End-to-end message delivery with session establishment

### Week 1: Session Layer Integration

- [ ] **Day 1-2**: Debug Noise handshake completion
  - Fix session state machine
  - Add handshake debugging logs
  - Test two-peer handshake completion
- [ ] **Day 3-4**: Integrate sessions with NetworkLayer
  - Route messages through established sessions
  - Handle session lifecycle events
  - Add session timeout handling
- [ ] **Day 5**: Network integration testing
  - Test commit propagation with real sessions
  - Verify encrypted message delivery
  - Test 3+ peer scenarios

### Week 2: Message Delivery & Synchronization

- [ ] **Day 1-2**: Commit synchronization
  - Ensure all members receive commits
  - Handle commit conflicts
  - Add commit ordering guarantees
- [ ] **Day 3-4**: Message queue & delivery
  - Implement reliable message delivery
  - Add retry logic for failed sends
  - Handle offline members gracefully
- [ ] **Day 5**: Multi-device testing
  - Test with 4-5 actors
  - Verify group state consistency
  - Test member join/leave scenarios

### Week 3: Network Reliability

- [ ] **Day 1-2**: Reconnection logic
  - Auto-reconnect on connection drop
  - Exponential backoff
  - State recovery after reconnect
- [ ] **Day 3-4**: Error handling
  - Network timeout handling
  - Invalid message handling
  - Peer disconnection handling
- [ ] **Day 5**: Network stress testing
  - Test with simulated packet loss
  - Test with connection interruptions
  - Performance benchmarking

### Week 4: Network Polish & Documentation

- [ ] **Day 1-2**: Activate privacy features
  - Enable message mixer
  - Enable per-channel identities
  - Test with real network
- [ ] **Day 3-4**: Network documentation
  - Architecture diagrams
  - API documentation
  - Troubleshooting guide
- [ ] **Day 5**: Integration test suite
  - 20+ network integration tests
  - Automated CI/CD tests
  - Performance benchmarks

**Success Criteria**:

- ✅ Noise handshakes complete successfully
- ✅ Messages delivered end-to-end
- ✅ Commits propagate to all members
- ✅ Network survives disconnections
- ✅ 100+ network tests passing

---

## Phase 2: Persistence Layer (Weeks 5-7)

**Goal**: Full state persistence and recovery

### Week 5: MLS Group Persistence

- [ ] **Day 1-2**: OpenMLS StorageProvider implementation
  - Implement 30+ trait methods
  - SQLite backend for group state
  - Transaction support
- [ ] **Day 3-4**: Group state serialization
  - Serialize/deserialize group state
  - Handle epoch transitions
  - Test with real groups
- [ ] **Day 5**: Group recovery testing
  - Test restart with existing groups
  - Verify epoch continuity
  - Test with multiple groups

### Week 6: Channel & Message Persistence

- [ ] **Day 1-2**: Channel state persistence
  - Save channel metadata
  - Save member lists
  - Save permissions
- [ ] **Day 3-4**: Message history
  - Store encrypted messages
  - Message indexing
  - Pagination support
- [ ] **Day 5**: Recovery testing
  - Full restart recovery
  - Verify message history
  - Test with large histories

### Week 7: Persistence Hardening

- [ ] **Day 1-2**: Transaction safety
  - Atomic state updates
  - Rollback on failure
  - Corruption detection
- [ ] **Day 3-4**: Migration system
  - Schema versioning
  - Automatic migrations
  - Backward compatibility
- [ ] **Day 5**: Persistence testing
  - Crash recovery tests
  - Corruption recovery tests
  - Performance benchmarks

**Success Criteria**:

- ✅ Groups survive restart
- ✅ Messages persist across sessions
- ✅ No data loss on crash
- ✅ Migration system working
- ✅ 50+ persistence tests passing

---

## Phase 3: Security Audit (Weeks 8-10)

**Goal**: Professional security validation

### Week 8: Pre-Audit Preparation

- [ ] **Day 1-2**: Audit code review
  - Remove all unwrap()/expect()
  - Fix remaining clippy warnings
  - Code cleanup
- [ ] **Day 3-4**: Security documentation
  - Threat model review
  - Security architecture docs
  - Known limitations doc
- [ ] **Day 5**: Select auditor
  - Get quotes from 3+ firms
  - Review credentials
  - Sign contract

### Week 9-10: Security Audit & Fixes

- [ ] **Week 9**: External audit
  - Provide codebase access
  - Answer auditor questions
  - Daily check-ins
- [ ] **Week 10**: Fix findings
  - Address critical issues
  - Address high priority issues
  - Re-test all fixes

**Success Criteria**:

- ✅ Professional audit complete
- ✅ All critical findings fixed
- ✅ Security certification obtained
- ✅ Audit report published

---

## Parallel Workstreams

### Ongoing: Testing & Quality

- Maintain 100% test pass rate
- Add tests for new features
- Keep code coverage >80%
- Daily CI/CD runs

### Ongoing: Documentation

- Keep docs in sync with code
- API documentation
- User guides
- Developer guides

### Ongoing: Performance

- Monitor memory usage
- Track message latency
- Optimize hot paths
- Benchmarking

---

## Risk Management

### High Risk Items

1. **Session handshake debugging** (Week 1)
   - Mitigation: Allocate extra time, add extensive logging
2. **OpenMLS StorageProvider complexity** (Week 5)
   - Mitigation: Study OpenMLS examples, get community help
3. **Security audit findings** (Week 10)
   - Mitigation: Pre-audit code review, allocate buffer time

### Dependencies

- Network layer completion blocks persistence testing
- Persistence completion blocks meaningful security audit
- All three must complete for beta release

---

## Beta Release Criteria

### Must Have

- ✅ End-to-end message delivery working
- ✅ Full persistence (groups + messages)
- ✅ Security audit complete
- ✅ No known critical bugs
- ✅ 1500+ tests passing

### Should Have

- ✅ Message mixer activated
- ✅ Per-channel identities working
- ✅ Network reliability (reconnect/retry)
- ✅ Basic CLI UX improvements

### Nice to Have

- ⚪ File attachments
- ⚪ Read receipts
- ⚪ Desktop/mobile apps

---

## Success Metrics

**Week 4 Checkpoint**:

- Network layer: 100% complete
- Tests passing: 1300+
- Session establishment: >95% success rate

**Week 7 Checkpoint**:

- Persistence: 100% complete
- Tests passing: 1400+
- Recovery success: 100%

**Week 10 Checkpoint**:

- Security audit: Complete
- Tests passing: 1500+
- Ready for beta release

---

## Next Actions (This Week)

### Monday-Tuesday: Session Debugging

1. Add extensive logging to session_manager.rs
2. Create minimal 2-peer handshake test
3. Debug why handshakes aren't completing
4. Fix session state machine

### Wednesday-Thursday: Network Integration

1. Route messages through sessions
2. Handle session events in NetworkLayer
3. Test commit propagation
4. Fix any blocking issues

### Friday: Testing & Review

1. Run full test suite
2. Fix any regressions
3. Document progress
4. Plan next week

---

**Last Updated**: December 6, 2025  
**Status**: ⚠️ IN PROGRESS - Phase 1 Starting  
**Next Milestone**: Week 1 Complete (Dec 13, 2025)
