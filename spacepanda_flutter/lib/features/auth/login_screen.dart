import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../providers/auth_providers.dart';
import '../home/home_screen.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final _passwordController = TextEditingController();
  final _usernameController = TextEditingController();
  bool _isCreatingProfile = false;

  @override
  void dispose() {
    _passwordController.dispose();
    _usernameController.dispose();
    super.dispose();
  }

  Future<void> _handleSubmit() async {
    final password = _passwordController.text.trim();
    if (password.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Please enter a password')),
      );
      return;
    }

    final username = _usernameController.text.trim();
    final usernameOrNull = username.isEmpty ? null : username;

    try {
      if (_isCreatingProfile) {
        await ref.read(profileCreatorProvider.notifier).createProfile(
              password: password,
              username: usernameOrNull,
            );
      } else {
        await ref.read(profileUnlockerProvider.notifier).unlock(
              password: password,
              username: usernameOrNull,
            );
      }

      if (mounted) {
        Navigator.of(context).pushReplacement(
          MaterialPageRoute(builder: (_) => const HomeScreen()),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final isLoading = ref.watch(profileCreatorProvider).isLoading ||
        ref.watch(profileUnlockerProvider).isLoading;

    return Scaffold(
      backgroundColor: const Color(0xFF36393F),
      body: Center(
        child: Container(
          constraints: const BoxConstraints(maxWidth: 400),
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Icon(
                Icons.lock_outline,
                size: 80,
                color: Color(0xFF5865F2),
              ),
              const SizedBox(height: 24),
              Text(
                _isCreatingProfile ? 'Create Profile' : 'Unlock Profile',
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 28,
                  fontWeight: FontWeight.bold,
                ),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                _isCreatingProfile
                    ? 'Create a new profile with a password'
                    : 'Enter your password to unlock',
                style: const TextStyle(
                  color: Color(0xFFB9BBBE),
                  fontSize: 14,
                ),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 32),
              TextField(
                controller: _usernameController,
                enabled: !isLoading,
                style: const TextStyle(color: Colors.white),
                decoration: InputDecoration(
                  labelText: 'Username (optional)',
                  labelStyle: const TextStyle(color: Color(0xFFB9BBBE)),
                  filled: true,
                  fillColor: const Color(0xFF40444B),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide: BorderSide.none,
                  ),
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _passwordController,
                enabled: !isLoading,
                obscureText: true,
                style: const TextStyle(color: Colors.white),
                decoration: InputDecoration(
                  labelText: 'Password',
                  labelStyle: const TextStyle(color: Color(0xFFB9BBBE)),
                  filled: true,
                  fillColor: const Color(0xFF40444B),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide: BorderSide.none,
                  ),
                ),
                onSubmitted: (_) => _handleSubmit(),
              ),
              const SizedBox(height: 24),
              ElevatedButton(
                onPressed: isLoading ? null : _handleSubmit,
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFF5865F2),
                  foregroundColor: Colors.white,
                  padding: const EdgeInsets.symmetric(vertical: 16),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(8),
                  ),
                ),
                child: isLoading
                    ? const SizedBox(
                        height: 20,
                        width: 20,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          valueColor: AlwaysStoppedAnimation(Colors.white),
                        ),
                      )
                    : Text(
                        _isCreatingProfile ? 'Create Profile' : 'Unlock',
                        style: const TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
              ),
              const SizedBox(height: 16),
              TextButton(
                onPressed: isLoading
                    ? null
                    : () {
                        setState(() {
                          _isCreatingProfile = !_isCreatingProfile;
                        });
                      },
                child: Text(
                  _isCreatingProfile
                      ? 'Already have a profile? Unlock'
                      : 'New user? Create profile',
                  style: const TextStyle(
                    color: Color(0xFF00AFF4),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
