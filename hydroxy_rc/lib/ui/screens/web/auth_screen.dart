import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/configs/web_provider.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/routes/routes_name.dart';
import 'package:hydroxy_rc/state/auth_bloc/auth_bloc.dart';

class AuthScreen extends StatefulWidget {
  final String? statusMessage;

  const AuthScreen({super.key, this.statusMessage});

  @override
  State<AuthScreen> createState() => _AuthScreenState();
}

class _AuthScreenState extends State<AuthScreen> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _usernameController;
  late final TextEditingController _passwordController;

  bool _obscurePassword = true;
  bool _isLoading = false;
  bool _isCheckingSession = true;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _usernameController = TextEditingController();
    _passwordController = TextEditingController();
    _isCheckingSession = widget.statusMessage == null;
    if (_isCheckingSession) {
      _checkExistingSession();
    }
  }

  Future<void> _checkExistingSession() async {
    try {
      final isAuthenticated = await ApiClient.instance.checkAuthStatus();
      if (!mounted) return;
      if (isAuthenticated) {
        Navigator.pushReplacementNamed(context, RoutesName.mainScreen);
        return;
      }
    } catch (_) {}
    if (mounted) {
      setState(() {
        _isCheckingSession = false;
      });
    }
  }

  @override
  void dispose() {
    _usernameController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  Future<void> _handleLogin() async {
    if (!(_formKey.currentState?.validate() ?? false)) {
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    final username = _usernameController.text.trim();
    final password = _passwordController.text.trim();
    final origin = WebUrlDetails.current().origin;

    try {
      final success = await ApiClient.instance.login(
        serverHostUrl: origin,
        username: username,
        password: password,
      );

      if (!mounted) return;

      if (success) {
        TextInput.finishAutofillContext(shouldSave: true);
        context.read<AuthBloc>().add(
          AuthEvent.login(username: username, password: password),
        );

        Navigator.pushReplacementNamed(context, RoutesName.mainScreen);
      } else {
        setState(() {
          _errorMessage = 'Invalid username or password.';
        });
      }
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _errorMessage = formatConnectionError(e, origin);
      });
    } finally {
      if (mounted) {
        setState(() {
          _isLoading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    if (_isCheckingSession) {
      return Scaffold(
        backgroundColor: colors.scaffoldBackgroundColor,
        body: Center(
          child: CircularProgressIndicator(
            valueColor: AlwaysStoppedAnimation<Color>(colors.accentGreenColor),
          ),
        ),
      );
    }

    return Scaffold(
      backgroundColor: colors.scaffoldBackgroundColor,
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Container(
            width: 360,
            padding: const EdgeInsets.all(28),
            decoration: BoxDecoration(
              color: colors.authCardBackgroundColor,
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: colors.authCardBorderColor),
            ),
            child: AutofillGroup(
              child: Form(
                key: _formKey,
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Icon(
                    Icons.image_not_supported_outlined,
                    size: 56,
                    color: colors.accentGreenColor,
                  ),
                  const SizedBox(height: 16),
                  Text(
                    "Hydroxy Web-UI Login",
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      color: colors.authTitleTextColor,
                      fontSize: 24,
                      fontWeight: FontWeight.bold,
                      letterSpacing: 0.5,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    "Sign in to manage your torrents",
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 14, color: colors.authSubtitleTextColor),
                  ),
                  const SizedBox(height: 28),

                  if (widget.statusMessage != null) ...[
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 10,
                      ),
                      margin: const EdgeInsets.only(bottom: 16),
                      decoration: BoxDecoration(
                        color: colors.badgeErrorBackgroundColor,
                        border: Border.all(color: Color.fromRGBO(colors.accentRedColor.red, colors.accentRedColor.green, colors.accentRedColor.blue, 0.5)),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Row(
                        children: [
                          Icon(
                            Icons.power_settings_new,
                            color: colors.accentRedColor,
                            size: 18,
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              widget.statusMessage!,
                              style: TextStyle(
                                color: colors.accentRedColor,
                                fontSize: 13,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],

                  if (_errorMessage != null) ...[
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 10,
                      ),
                      decoration: BoxDecoration(
                        color: colors.badgeErrorBackgroundColor,
                        border: Border.all(color: Color.fromRGBO(colors.accentRedColor.red, colors.accentRedColor.green, colors.accentRedColor.blue, 0.5)),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Row(
                        children: [
                          Icon(
                            Icons.error_outline,
                            color: colors.accentRedColor,
                            size: 20,
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              _errorMessage!,
                              style: TextStyle(
                                color: colors.accentRedColor,
                                fontSize: 13,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 16),
                  ],

                  TextFormField(
                    controller: _usernameController,
                    keyboardType: TextInputType.text,
                    textInputAction: TextInputAction.next,
                    autofillHints: const [AutofillHints.username],
                    style: TextStyle(color: colors.authInputTextColor),
                    decoration: InputDecoration(
                      labelText: "Username",
                      labelStyle: TextStyle(color: colors.authSubtitleTextColor),
                      prefixIcon: Icon(Icons.person_outline, color: colors.authSubtitleTextColor),
                      filled: true,
                      fillColor: colors.authInputBackgroundColor,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputBorderColor),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputBorderColor),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputActiveBorderColor),
                      ),
                    ),
                    validator: (v) => v == null || v.trim().isEmpty
                        ? 'Username required'
                        : null,
                  ),

                  const SizedBox(height: 18),

                  TextFormField(
                    controller: _passwordController,
                    obscureText: _obscurePassword,
                    keyboardType: TextInputType.visiblePassword,
                    textInputAction: TextInputAction.done,
                    autofillHints: const [AutofillHints.password],
                    style: TextStyle(color: colors.authInputTextColor),
                    onFieldSubmitted: (_) => _handleLogin(),
                    decoration: InputDecoration(
                      labelText: "Password",
                      labelStyle: TextStyle(color: colors.authSubtitleTextColor),
                      prefixIcon: Icon(Icons.lock_outline, color: colors.authSubtitleTextColor),
                      filled: true,
                      fillColor: colors.authInputBackgroundColor,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputBorderColor),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputBorderColor),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                        borderSide: BorderSide(color: colors.authInputActiveBorderColor),
                      ),
                      suffixIcon: IconButton(
                        icon: Icon(
                          _obscurePassword
                              ? Icons.visibility_off_outlined
                              : Icons.visibility_outlined,
                          color: colors.authSubtitleTextColor,
                        ),
                        onPressed: () {
                          setState(() => _obscurePassword = !_obscurePassword);
                        },
                      ),
                    ),
                    validator: (v) => v == null || v.trim().isEmpty
                        ? 'Password required'
                        : null,
                  ),

                  const SizedBox(height: 28),

                  SizedBox(
                    height: 48,
                    child: ElevatedButton(
                      onPressed: _isLoading ? null : _handleLogin,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: colors.authButtonBackgroundColor,
                        foregroundColor: colors.authButtonTextColor,
                        elevation: 0,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(10),
                        ),
                      ),
                      child: _isLoading
                          ? SizedBox(
                              height: 20,
                              width: 20,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: colors.authButtonTextColor,
                              ),
                            )
                          : Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Icon(Icons.login, size: 19, color: colors.authButtonTextColor),
                                const SizedBox(width: 8),
                                Text(
                                  'Login',
                                  style: TextStyle(
                                    color: colors.authButtonTextColor,
                                    fontSize: 16,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ],
                            ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    ),
  );
}
}
